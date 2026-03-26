pub mod builder;
pub mod sequential;
pub mod independent;

use bashli_core::*;
use bashli_budget::BudgetTracker;
use bashli_extract::ExtractorRegistry;
use bashli_runner::CommandRunner;
use bashli_steps::{StepContext, StepRegistry};
use bashli_transforms::TransformRegistry;
use bashli_vars::VarStore;
pub use builder::EngineBuilder;

/// The bashli execution engine.
pub struct Engine {
    step_registry: StepRegistry,
    transform_registry: TransformRegistry,
    extractor_registry: ExtractorRegistry,
    runner: CommandRunner,
    settings: GlobalSettings,
}

impl Engine {
    /// Execute a full TaskSpec, returning the structured result.
    pub async fn run(&self, spec: TaskSpec) -> TaskResult {
        let start = std::time::Instant::now();

        // 1. Validate
        if let Err(errors) = validate_task_spec(&spec) {
            let message = errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ");
            return TaskResult {
                ok: false,
                duration_ms: start.elapsed().as_millis() as u64,
                variables: Default::default(),
                steps: vec![],
                warnings: vec![],
                error: Some(TaskError {
                    step_index: 0,
                    kind: ErrorKind::ValidationError,
                    message,
                }),
            };
        }

        // Merge spec settings with engine defaults
        let settings = merge_settings(&self.settings, &spec.settings);

        // 2. Initialize VarStore
        let mut vars = VarStore::new();
        vars.init_system_vars();

        // Process let_vars
        if let Some(ref let_vars) = spec.let_vars {
            for (name, value_template) in let_vars {
                match vars.interpolate(value_template, false) {
                    Ok(value) => vars.set(name, serde_json::Value::String(value)),
                    Err(e) => {
                        return TaskResult {
                            ok: false,
                            duration_ms: start.elapsed().as_millis() as u64,
                            variables: Default::default(),
                            steps: vec![],
                            warnings: vec![],
                            error: Some(TaskError {
                                step_index: 0,
                                kind: ErrorKind::UndefinedVariable(e.to_string()),
                                message: format!("failed to resolve let_vars: {e}"),
                            }),
                        };
                    }
                }
            }
        }

        // 3. Initialize BudgetTracker
        let mut budget = if let Some(ref tb) = settings.token_budget {
            BudgetTracker::new(tb, spec.steps.len())
        } else if let Some(max_tokens) = settings.max_output_tokens {
            let tb = TokenBudget {
                max_tokens,
                allocation: BudgetAllocation::Equal,
                overflow: OverflowStrategy::Truncate,
            };
            BudgetTracker::new(&tb, spec.steps.len())
        } else {
            BudgetTracker::unlimited()
        };

        // 4. Dispatch to mode-specific executor
        let mut ctx = StepContext {
            vars: &mut vars,
            runner: &self.runner,
            budget: &mut budget,
            transforms: &self.transform_registry,
            extractors: &self.extractor_registry,
            settings: &settings,
        };

        let (steps, exec_error) = match spec.mode {
            ExecutionMode::Sequential => {
                sequential::run_sequential(&spec.steps, &self.step_registry, &mut ctx).await
            }
            ExecutionMode::Independent => {
                independent::run_independent(&spec.steps, &self.step_registry, &mut ctx).await
            }
            ExecutionMode::Parallel | ExecutionMode::ParallelN(_) => {
                // Not yet implemented — fall back to independent
                independent::run_independent(&spec.steps, &self.step_registry, &mut ctx).await
            }
        };

        // 5. Build TaskResult
        let ok = exec_error.is_none()
            && steps.iter().all(|s| {
                s.exit_code.map(|c| c == 0).unwrap_or(true)
            });

        let error = exec_error.map(|(step_index, err)| {
            let (kind, message) = match &err {
                ExecError::NonZeroExit(code) => (ErrorKind::NonZeroExit(*code), err.to_string()),
                ExecError::Timeout(ms) => (ErrorKind::Timeout, format!("step {step_index} timed out after {ms}ms")),
                ExecError::AssertionFailed(msg) => (ErrorKind::AssertionFailed, msg.clone()),
                ExecError::UndefinedVariable(var) => (ErrorKind::UndefinedVariable(var.clone()), err.to_string()),
                ExecError::ParseError(msg) => (ErrorKind::ParseError, msg.clone()),
                ExecError::IoError(e) => (ErrorKind::IoError, e.to_string()),
                ExecError::BudgetExhausted => (ErrorKind::Timeout, "budget exhausted".into()),
                _ => (ErrorKind::IoError, err.to_string()),
            };
            TaskError { step_index, kind, message }
        });

        // Collect variables
        let variables = if let Some(ref summary_keys) = spec.summary {
            ctx.vars.export_summary(summary_keys)
        } else {
            ctx.vars.export_all()
        };

        // Apply verbosity filtering to steps
        let steps = filter_steps_by_verbosity(steps, &settings.verbosity, spec.summary.is_some());

        TaskResult {
            ok: ok && error.is_none(),
            duration_ms: start.elapsed().as_millis() as u64,
            variables,
            steps,
            warnings: vec![],
            error,
        }
    }
}

fn merge_settings(engine: &GlobalSettings, spec: &GlobalSettings) -> GlobalSettings {
    // Spec settings override engine defaults
    GlobalSettings {
        stderr: spec.stderr.clone(),
        stdout: spec.stdout.clone(),
        max_output_tokens: spec.max_output_tokens.or(engine.max_output_tokens),
        timeout_ms: if spec.timeout_ms != 30_000 { spec.timeout_ms } else { engine.timeout_ms },
        cwd: spec.cwd.clone().or_else(|| engine.cwd.clone()),
        env: spec.env.clone().or_else(|| engine.env.clone()),
        shell: spec.shell.clone().or_else(|| engine.shell.clone()),
        verbosity: spec.verbosity.clone(),
        token_budget: spec.token_budget.clone().or_else(|| engine.token_budget.clone()),
        read_only: spec.read_only || engine.read_only,
        allowed_paths: spec.allowed_paths.clone().or_else(|| engine.allowed_paths.clone()),
    }
}

fn filter_steps_by_verbosity(steps: Vec<StepResult>, verbosity: &Verbosity, has_summary: bool) -> Vec<StepResult> {
    match verbosity {
        Verbosity::Minimal => {
            // Omit steps array entirely when minimal
            vec![]
        }
        Verbosity::Normal if has_summary => {
            // In summary mode with normal verbosity, strip stdout from steps
            steps.into_iter().map(|mut s| {
                s.stdout = None;
                s.stderr = None;
                s
            }).collect()
        }
        _ => steps,
    }
}
