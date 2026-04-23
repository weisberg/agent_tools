use std::path::{Path, PathBuf};

use docli_core::DocliError;

/// Check if soffice is available.
pub fn is_available() -> bool {
    find_soffice().is_some()
}

/// Find the soffice binary path.
pub fn find_soffice() -> Option<PathBuf> {
    // Check PATH first
    if std::process::Command::new("soffice")
        .arg("--version")
        .output()
        .is_ok()
    {
        return Some(PathBuf::from("soffice"));
    }
    // macOS application bundle
    let macos_path = PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice");
    if macos_path.exists() {
        return Some(macos_path);
    }
    None
}

/// Convert DOCX to PDF using LibreOffice.
/// Returns path to the generated PDF.
pub fn docx_to_pdf(docx_path: &Path, output_dir: &Path) -> Result<PathBuf, DocliError> {
    let soffice = find_soffice().ok_or_else(|| DocliError::DependencyMissing {
        dependency: "soffice (LibreOffice)".to_string(),
    })?;

    let output = std::process::Command::new(&soffice)
        .args(["--headless", "--convert-to", "pdf", "--outdir"])
        .arg(output_dir)
        .arg(docx_path)
        .output()
        .map_err(|e| DocliError::DependencyMissing {
            dependency: format!("soffice: {}", e),
        })?;

    if !output.status.success() {
        return Err(DocliError::CommitFailed {
            message: format!(
                "soffice conversion failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    // Output PDF has same stem as input but .pdf extension
    let stem = docx_path.file_stem().unwrap_or_default();
    let pdf_path = output_dir.join(format!("{}.pdf", stem.to_string_lossy()));

    if !pdf_path.exists() {
        return Err(DocliError::CommitFailed {
            message: "soffice did not produce expected PDF output".to_string(),
        });
    }

    Ok(pdf_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available_runs() {
        // Just verify the function runs without panicking.
        let _available = is_available();
    }

    #[test]
    fn test_find_soffice_runs() {
        // May return None if LibreOffice is not installed.
        let _path = find_soffice();
    }
}
