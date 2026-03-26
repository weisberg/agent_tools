use async_trait::async_trait;
use bashli_core::*;
use crate::context::StepContext;
use crate::StepExecutor;

pub struct AssertExecutor {
    step: AssertStep,
}

impl AssertExecutor {
    pub fn new(step: AssertStep) -> Self {
        Self { step }
    }
}

#[async_trait]
impl StepExecutor for AssertExecutor {
    fn kind(&self) -> StepKind {
        StepKind::Assert
    }

    async fn execute(&self, index: usize, ctx: &mut StepContext<'_>) -> Result<StepResult, ExecError> {
        let start = std::time::Instant::now();

        // Resolve the variable value
        let var_value = ctx.vars.resolve(&self.step.var)
            .map_err(|e| ExecError::VarError(e.to_string()))?;

        let value_str = match &var_value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => String::new(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };

        let passed = self.step.condition.evaluate(&value_str);
        let duration_ms = start.elapsed().as_millis() as u64;

        if passed {
            let mut result = StepResult::new(index, StepKind::Assert, duration_ms);
            result.note = Some("passed".to_string());
            return Ok(result);
        }

        // Assertion failed
        let message = if let Some(ref msg_template) = self.step.message {
            ctx.vars.interpolate(msg_template, false)
                .unwrap_or_else(|_| msg_template.clone())
        } else {
            format!("assertion failed: {} did not satisfy condition", self.step.var)
        };

        let fail_action = self.step.on_fail.as_ref()
            .cloned()
            .unwrap_or(AssertFailAction::Abort);

        match fail_action {
            AssertFailAction::Abort => {
                Err(ExecError::AssertionFailed(message))
            }
            AssertFailAction::Warn => {
                let mut result = StepResult::new(index, StepKind::Assert, duration_ms);
                result.note = Some(format!("warning: {message}"));
                Ok(result)
            }
            AssertFailAction::SkipRest => {
                let mut result = StepResult::new(index, StepKind::Assert, duration_ms);
                result.note = Some(format!("skip_rest: {message}"));
                Ok(result)
            }
            AssertFailAction::Fallback(_) => {
                // Fallback step execution not yet implemented
                Err(ExecError::AssertionFailed(message))
            }
        }
    }
}
