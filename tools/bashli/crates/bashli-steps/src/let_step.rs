use async_trait::async_trait;
use bashli_core::*;
use crate::context::StepContext;
use crate::StepExecutor;

pub struct LetExecutor {
    step: LetStep,
}

impl LetExecutor {
    pub fn new(step: LetStep) -> Self {
        Self { step }
    }
}

#[async_trait]
impl StepExecutor for LetExecutor {
    fn kind(&self) -> StepKind {
        StepKind::Let
    }

    async fn execute(&self, index: usize, ctx: &mut StepContext<'_>) -> Result<StepResult, ExecError> {
        let start = std::time::Instant::now();
        let mut captured = Vec::new();

        for (name, value_template) in &self.step.bindings {
            let value = ctx.vars.interpolate(value_template, false)
                .map_err(|e| ExecError::VarError(e.to_string()))?;
            ctx.vars.set(name, serde_json::Value::String(value));
            captured.push(name.clone());
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let mut result = StepResult::new(index, StepKind::Let, duration_ms);
        result.captured = Some(captured);
        result.note = Some("variables set".to_string());
        Ok(result)
    }
}
