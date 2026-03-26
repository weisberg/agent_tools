use bashli_core::{ExecError, Step, StructuredStep};
use crate::StepExecutor;

/// Registry that resolves Step variants to executors.
pub struct StepRegistry;

impl StepRegistry {
    pub fn new() -> Self {
        Self
    }

    /// Resolve a Step to its executor.
    pub fn resolve(&self, step: &Step) -> Result<Box<dyn StepExecutor>, ExecError> {
        match step {
            Step::BareCmd(cmd) => {
                Ok(Box::new(crate::cmd::CmdExecutor::from_bare(cmd.clone())))
            }
            Step::Structured(s) => match s {
                StructuredStep::Cmd(cmd_step) => {
                    Ok(Box::new(crate::cmd::CmdExecutor::new(cmd_step.clone())))
                }
                StructuredStep::Let(let_step) => {
                    Ok(Box::new(crate::let_step::LetExecutor::new(let_step.clone())))
                }
                StructuredStep::Assert(assert_step) => {
                    Ok(Box::new(crate::assert::AssertExecutor::new(assert_step.clone())))
                }
                StructuredStep::Write(write_step) => {
                    Ok(Box::new(crate::write::WriteExecutor::new(write_step.write.clone())))
                }
                StructuredStep::Read(read_step) => {
                    Ok(Box::new(crate::read::ReadExecutor::new(read_step.read.clone())))
                }
                StructuredStep::If(_) => {
                    Err(ExecError::NotYetSupported("if".to_string()))
                }
                StructuredStep::ForEach(_) => {
                    Err(ExecError::NotYetSupported("for_each".to_string()))
                }
                StructuredStep::Extension(ext) => {
                    Err(ExecError::ExtensionError {
                        kind: ext.extension.kind.clone(),
                        message: "extension steps not yet supported".to_string(),
                    })
                }
            },
        }
    }
}

impl Default for StepRegistry {
    fn default() -> Self {
        Self::new()
    }
}
