use std::path::{Path, PathBuf};

use docli_core::DocliError;

/// Check if pdftoppm is available.
pub fn is_available() -> bool {
    std::process::Command::new("pdftoppm")
        .arg("-h")
        .output()
        .is_ok()
}

/// Render PDF pages to PNG images.
/// Returns paths to generated image files.
pub fn pdf_to_images(
    pdf_path: &Path,
    output_dir: &Path,
    prefix: &str,
    dpi: u32,
) -> Result<Vec<PathBuf>, DocliError> {
    let output = std::process::Command::new("pdftoppm")
        .args(["-png", "-r", &dpi.to_string()])
        .arg(pdf_path)
        .arg(output_dir.join(prefix))
        .output()
        .map_err(|e| DocliError::DependencyMissing {
            dependency: format!("pdftoppm: {}", e),
        })?;

    if !output.status.success() {
        return Err(DocliError::CommitFailed {
            message: format!(
                "pdftoppm failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    // Collect generated image files (pdftoppm creates prefix-01.png, prefix-02.png, etc.)
    let mut images = Vec::new();
    if let Ok(entries) = std::fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix) && n.ends_with(".png"))
            {
                images.push(path);
            }
        }
    }
    images.sort();
    Ok(images)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available_runs() {
        let _available = is_available();
    }
}
