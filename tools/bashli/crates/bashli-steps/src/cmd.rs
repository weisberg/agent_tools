use async_trait::async_trait;
use bashli_core::*;
use bashli_runner::RunOpts;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::context::StepContext;
use crate::StepExecutor;

pub struct CmdExecutor {
    step: CmdStep,
}

impl CmdExecutor {
    pub fn new(step: CmdStep) -> Self {
        Self { step }
    }

    pub fn from_bare(cmd: String) -> Self {
        Self {
            step: CmdStep {
                cmd,
                capture: None,
                transform: None,
                extract: None,
                stdout: None,
                stderr: None,
                stdin: None,
                timeout_ms: None,
                cwd: None,
                env: None,
                limit: None,
                retry: None,
                on_failure: None,
                verbose: None,
            },
        }
    }
}

#[async_trait]
impl StepExecutor for CmdExecutor {
    fn kind(&self) -> StepKind {
        StepKind::Cmd
    }

    async fn execute(&self, index: usize, ctx: &mut StepContext<'_>) -> Result<StepResult, ExecError> {
        let start = std::time::Instant::now();

        // 1. Interpolate command, cwd, env, stdin
        let cmd = ctx.vars.interpolate(&self.step.cmd, true)
            .map_err(|e| ExecError::VarError(e.to_string()))?;

        let cwd = match self.step.cwd.as_ref().or(ctx.settings.cwd.as_ref()) {
            Some(c) => {
                let interpolated = ctx.vars.interpolate(c, false)
                    .map_err(|e| ExecError::VarError(e.to_string()))?;
                Some(PathBuf::from(interpolated))
            }
            None => None,
        };

        let mut env = BTreeMap::new();
        if let Some(ref global_env) = ctx.settings.env {
            for (k, v) in global_env {
                let val = ctx.vars.interpolate(v, false)
                    .map_err(|e| ExecError::VarError(e.to_string()))?;
                env.insert(k.clone(), val);
            }
        }
        if let Some(ref step_env) = self.step.env {
            for (k, v) in step_env {
                let val = ctx.vars.interpolate(v, false)
                    .map_err(|e| ExecError::VarError(e.to_string()))?;
                env.insert(k.clone(), val);
            }
        }

        let stdin_data = match &self.step.stdin {
            Some(s) => {
                let interpolated = ctx.vars.interpolate(s, false)
                    .map_err(|e| ExecError::VarError(e.to_string()))?;
                Some(interpolated.into_bytes())
            }
            None => None,
        };

        let timeout_ms = self.step.timeout_ms.unwrap_or(ctx.settings.timeout_ms);
        let stderr_mode = self.step.stderr.clone().unwrap_or_else(|| ctx.settings.stderr.clone());
        let stdout_mode = self.step.stdout.clone().unwrap_or_else(|| ctx.settings.stdout.clone());

        // 2. Run command
        let opts = RunOpts {
            cwd,
            env,
            stdout_mode,
            stderr_mode,
            stdin_data,
            timeout: Some(Duration::from_millis(timeout_ms)),
        };

        let raw = ctx.runner.run(&cmd, &opts).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        // 3. Get stdout as string
        let stdout_str = String::from_utf8_lossy(&raw.stdout).to_string();
        let stderr_str = if raw.stderr.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&raw.stderr).to_string())
        };

        // 5-6. Apply transforms, then capture and extract
        let transformed = if let Some(ref transform) = self.step.transform {
            let result = ctx.transforms.apply(&stdout_str, transform)
                .map_err(|e| ExecError::TransformError(e.to_string()))?;
            value_to_string(&result)
        } else {
            stdout_str.clone()
        };

        // Capture
        let mut captured = Vec::new();
        if let Some(ref var_name) = self.step.capture {
            let value = serde_json::Value::String(transformed.clone());
            ctx.vars.set(var_name, value);
            captured.push(var_name.clone());
        }

        // Extract
        if let Some(ref extractions) = self.step.extract {
            for (var_name, extraction) in extractions {
                let value = ctx.extractors.apply(&transformed, extraction)
                    .map_err(|e| ExecError::ExtractionError(e.to_string()))?;
                ctx.vars.set(var_name, value);
                captured.push(var_name.clone());
            }
        }

        // 7. Apply per-step limit
        let (output, truncated, truncated_lines) = apply_limit(&transformed, &self.step.limit);

        // 8. Charge budget
        let budget_result = ctx.budget.charge(index, &output);
        let (final_output, was_truncated, trunc_lines) = match budget_result {
            bashli_budget::BudgetResult::Accepted(s) => (Some(s), truncated, truncated_lines),
            bashli_budget::BudgetResult::Truncated { output, lines_dropped } => {
                (Some(output), true, Some(lines_dropped))
            }
            bashli_budget::BudgetResult::Dropped => (None, true, None),
            bashli_budget::BudgetResult::Abort => return Err(ExecError::BudgetExhausted),
        };

        // Update system variables
        ctx.vars.set("$_PREV_EXIT", serde_json::Value::Number(raw.exit_code.into()));
        let prev_stdout = if stdout_str.len() > 4096 {
            stdout_str[..4096].to_string()
        } else {
            stdout_str.clone()
        };
        ctx.vars.set("$_PREV_STDOUT", serde_json::Value::String(prev_stdout));

        // 9. Build StepResult
        let mut result = StepResult::new(index, StepKind::Cmd, duration_ms);
        result.exit_code = Some(raw.exit_code);
        result.stdout = final_output;
        result.stderr = stderr_str;
        result.truncated = was_truncated;
        result.truncated_lines = trunc_lines;
        if !captured.is_empty() {
            result.captured = Some(captured);
        }

        // 10. Check exit code
        if raw.exit_code != 0 {
            // Try on_failure if set
            if self.step.on_failure.is_some() {
                result.note = Some(format!("command failed with exit code {}, on_failure not yet implemented", raw.exit_code));
            }
            return Ok(result);
        }

        Ok(result)
    }
}

fn apply_limit(input: &str, limit: &Option<LimitSpec>) -> (String, bool, Option<usize>) {
    let limit = match limit {
        Some(l) => l,
        None => return (input.to_string(), false, None),
    };

    if let Some(max_lines) = limit.max_lines {
        let lines: Vec<&str> = input.lines().collect();
        if lines.len() <= max_lines {
            return (input.to_string(), false, None);
        }

        let dropped = lines.len() - max_lines;
        let truncated = match limit.strategy {
            TruncationStrategy::Head => lines[..max_lines].join("\n"),
            TruncationStrategy::Tail => {
                let start = lines.len().saturating_sub(max_lines);
                lines[start..].join("\n")
            }
            TruncationStrategy::Smart => {
                let half = max_lines / 2;
                let tail_start = lines.len().saturating_sub(max_lines - half);
                let head: Vec<&str> = lines[..half].to_vec();
                let tail: Vec<&str> = lines[tail_start..].to_vec();
                format!(
                    "{}\n... [truncated {} lines] ...\n{}",
                    head.join("\n"),
                    dropped,
                    tail.join("\n")
                )
            }
            TruncationStrategy::Filter(ref pattern) => {
                if let Ok(re) = regex::Regex::new(pattern) {
                    let filtered: Vec<&str> = lines.iter()
                        .filter(|l| re.is_match(l))
                        .copied()
                        .collect();
                    filtered.join("\n")
                } else {
                    input.to_string()
                }
            }
        };
        return (truncated, true, Some(dropped));
    }

    if let Some(max_bytes) = limit.max_bytes {
        if input.len() > max_bytes {
            let truncated = input[..max_bytes].to_string();
            return (truncated, true, None);
        }
    }

    (input.to_string(), false, None)
}

fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}
