use async_trait::async_trait;
use bashli_core::*;
use crate::context::StepContext;
use crate::StepExecutor;

pub struct ReadExecutor {
    step: ReadStep,
}

impl ReadExecutor {
    pub fn new(step: ReadStep) -> Self {
        Self { step }
    }
}

#[async_trait]
impl StepExecutor for ReadExecutor {
    fn kind(&self) -> StepKind {
        StepKind::Read
    }

    async fn execute(&self, index: usize, ctx: &mut StepContext<'_>) -> Result<StepResult, ExecError> {
        let start = std::time::Instant::now();

        // Interpolate path
        let path_str = ctx.vars.interpolate(&self.step.path, false)
            .map_err(|e| ExecError::VarError(e.to_string()))?;

        // Read file
        let content = std::fs::read_to_string(&path_str)?;

        // Apply transform if specified
        let transformed = if let Some(ref transform) = self.step.transform {
            let result = ctx.transforms.apply(&content, transform)
                .map_err(|e| ExecError::TransformError(e.to_string()))?;
            match result {
                serde_json::Value::String(s) => s,
                other => serde_json::to_string(&other).unwrap_or_default(),
            }
        } else {
            content
        };

        // Capture into variable
        ctx.vars.set(&self.step.capture, serde_json::Value::String(transformed));

        let duration_ms = start.elapsed().as_millis() as u64;
        let mut result = StepResult::new(index, StepKind::Read, duration_ms);
        result.captured = Some(vec![self.step.capture.clone()]);
        result.note = Some(format!("read {}", path_str));
        Ok(result)
    }
}
