use async_trait::async_trait;
use bashli_core::*;
use crate::context::StepContext;
use crate::StepExecutor;
use std::path::Path;

pub struct WriteExecutor {
    step: WriteStep,
}

impl WriteExecutor {
    pub fn new(step: WriteStep) -> Self {
        Self { step }
    }
}

#[async_trait]
impl StepExecutor for WriteExecutor {
    fn kind(&self) -> StepKind {
        StepKind::Write
    }

    async fn execute(&self, index: usize, ctx: &mut StepContext<'_>) -> Result<StepResult, ExecError> {
        let start = std::time::Instant::now();

        // Check read-only mode
        if ctx.settings.read_only {
            return Err(ExecError::IoError(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "write operations are disabled in read-only mode",
            )));
        }

        // Interpolate path and content
        let path_str = ctx.vars.interpolate(&self.step.path, false)
            .map_err(|e| ExecError::VarError(e.to_string()))?;
        let content = ctx.vars.interpolate(&self.step.content, false)
            .map_err(|e| ExecError::VarError(e.to_string()))?;

        let path = Path::new(&path_str);

        // Check allowed paths
        if let Some(ref allowed) = ctx.settings.allowed_paths {
            let path_canonical = path.to_string_lossy();
            let allowed_match = allowed.iter().any(|pattern| {
                path_canonical.starts_with(pattern) || glob_match(pattern, &path_canonical)
            });
            if !allowed_match {
                return Err(ExecError::IoError(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("path '{}' is not in allowed paths", path_str),
                )));
            }
        }

        // Create parent dirs if requested
        if self.step.mkdir {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Write based on mode
        match self.step.mode {
            WriteMode::Create => {
                std::fs::write(path, &content)?;
            }
            WriteMode::Append => {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;
                file.write_all(content.as_bytes())?;
            }
            WriteMode::Atomic => {
                let temp_path = path.with_extension("tmp");
                std::fs::write(&temp_path, &content)?;
                std::fs::rename(&temp_path, path)?;
            }
            WriteMode::CreateNew => {
                if path.exists() {
                    return Err(ExecError::IoError(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        format!("file already exists: {}", path_str),
                    )));
                }
                std::fs::write(path, &content)?;
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let mut result = StepResult::new(index, StepKind::Write, duration_ms);
        result.note = Some(format!("wrote {}", path_str));
        Ok(result)
    }
}

/// Simple glob matching (just prefix and * support).
fn glob_match(pattern: &str, path: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        path.starts_with(prefix)
    } else {
        pattern == path
    }
}
