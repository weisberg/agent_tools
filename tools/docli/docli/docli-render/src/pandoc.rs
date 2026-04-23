use std::path::Path;

use docli_core::DocliError;

/// Check if pandoc is available in PATH.
pub fn is_available() -> bool {
    std::process::Command::new("pandoc")
        .arg("--version")
        .output()
        .is_ok()
}

/// Get pandoc version string.
pub fn version() -> Option<String> {
    let output = std::process::Command::new("pandoc")
        .arg("--version")
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().next().map(|s| s.to_string())
}

/// Convert a DOCX file to markdown using pandoc.
pub fn docx_to_markdown(docx_path: &Path) -> Result<String, DocliError> {
    let output = std::process::Command::new("pandoc")
        .args(["-f", "docx", "-t", "markdown", "--wrap=none"])
        .arg(docx_path)
        .output()
        .map_err(|e| DocliError::DependencyMissing {
            dependency: format!("pandoc: {}", e),
        })?;
    if !output.status.success() {
        return Err(DocliError::InvalidDocx {
            message: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Convert DOCX to markdown with tracked changes visible.
pub fn docx_to_markdown_with_changes(docx_path: &Path) -> Result<String, DocliError> {
    let output = std::process::Command::new("pandoc")
        .args([
            "-f",
            "docx",
            "-t",
            "markdown",
            "--wrap=none",
            "--track-changes=all",
        ])
        .arg(docx_path)
        .output()
        .map_err(|e| DocliError::DependencyMissing {
            dependency: format!("pandoc: {}", e),
        })?;
    if !output.status.success() {
        return Err(DocliError::InvalidDocx {
            message: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Convert DOCX to plain text.
pub fn docx_to_text(docx_path: &Path) -> Result<String, DocliError> {
    let output = std::process::Command::new("pandoc")
        .args(["-f", "docx", "-t", "plain", "--wrap=none"])
        .arg(docx_path)
        .output()
        .map_err(|e| DocliError::DependencyMissing {
            dependency: format!("pandoc: {}", e),
        })?;
    if !output.status.success() {
        return Err(DocliError::InvalidDocx {
            message: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available_runs() {
        // Just verify the function runs without panicking.
        // It may return true or false depending on the environment.
        let _available = is_available();
    }

    #[test]
    fn test_version_runs() {
        // version() returns None if pandoc is not installed, Some otherwise.
        let _v = version();
    }
}
