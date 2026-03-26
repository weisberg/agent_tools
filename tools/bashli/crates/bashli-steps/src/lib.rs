pub mod context;
pub mod registry;
pub mod cmd;
pub mod let_step;
pub mod assert;
pub mod write;
pub mod read;

pub use context::StepContext;
pub use registry::StepRegistry;

use async_trait::async_trait;
use bashli_core::{ExecError, StepKind, StepResult, ValidationError};

/// The trait every step type must implement.
#[async_trait]
pub trait StepExecutor: Send + Sync {
    /// Returns the step kind for StepResult.kind
    fn kind(&self) -> StepKind;

    /// Validate the step's configuration before execution.
    fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }

    /// Execute the step, returning a StepResult.
    async fn execute(&self, index: usize, ctx: &mut StepContext<'_>) -> Result<StepResult, ExecError>;
}
