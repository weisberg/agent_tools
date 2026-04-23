use schemars::JsonSchema;
use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use xli_core::XliError;

#[derive(Clone, Debug, Default, PartialEq, Serialize, JsonSchema)]
pub struct RecalcResult {
    pub duration_ms: u64,
    pub libreoffice_version: Option<String>,
    pub warnings: Vec<String>,
}

pub fn recalc(path: &Path, timeout_secs: u64) -> Result<RecalcResult, XliError> {
    if !path.exists() {
        return Err(XliError::FileNotFound {
            path: path.display().to_string(),
        });
    }

    let started = Instant::now();
    let version = libreoffice_version().ok();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| XliError::RecalcFailed {
            details: "Workbook path must include a file name".to_string(),
        })?;

    let mut child = Command::new("libreoffice")
        .args([
            "--headless",
            "--convert-to",
            "xlsx",
            file_name,
            "--outdir",
            parent.to_str().unwrap_or("."),
        ])
        .current_dir(parent)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| XliError::RecalcFailed {
            details: format!("Unable to start LibreOffice: {error}"),
        })?;

    let timeout = Duration::from_secs(timeout_secs);
    loop {
        if let Some(status) = child.try_wait().map_err(io_error)? {
            if !status.success() {
                let output = child.wait_with_output().map_err(io_error)?;
                return Err(XliError::RecalcFailed {
                    details: String::from_utf8_lossy(&output.stderr).trim().to_string(),
                });
            }

            return Ok(RecalcResult {
                duration_ms: started.elapsed().as_millis() as u64,
                libreoffice_version: version,
                warnings: Vec::new(),
            });
        }

        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(XliError::RecalcTimeout { timeout_secs });
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn libreoffice_version() -> Result<String, XliError> {
    let output = Command::new("libreoffice")
        .arg("--version")
        .output()
        .map_err(|error| XliError::RecalcFailed {
            details: format!("LibreOffice is not available: {error}"),
        })?;

    if !output.status.success() {
        return Err(XliError::RecalcFailed {
            details: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn io_error(error: std::io::Error) -> XliError {
    XliError::RecalcFailed {
        details: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::recalc;
    use std::fs;
    use tempfile::tempdir;
    use xli_core::XliError;

    #[test]
    fn missing_libreoffice_returns_structured_error_or_timeout() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("book.xlsx");
        fs::write(&path, b"not really xlsx").expect("write");

        let result = recalc(&path, 1);
        assert!(matches!(
            result,
            Err(XliError::RecalcFailed { .. }) | Err(XliError::RecalcTimeout { .. })
        ));
    }
}
