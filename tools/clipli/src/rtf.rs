use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, thiserror::Error)]
pub enum RtfError {
    #[error("textutil not found — RTF conversion requires macOS")]
    TextutilNotFound,
    #[error("RTF conversion failed: {0}")]
    ConversionFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl RtfError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::TextutilNotFound => "RTF_TEXTUTIL_NOT_FOUND",
            Self::ConversionFailed(_) => "RTF_CONVERSION_FAILED",
            Self::Io(_) => "RTF_IO_ERROR",
        }
    }
}

/// Convert RTF bytes to HTML using macOS textutil.
pub fn rtf_to_html(rtf_bytes: &[u8]) -> Result<String, RtfError> {
    let mut child = match Command::new("/usr/bin/textutil")
        .args(["-convert", "html", "-stdin", "-stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(RtfError::TextutilNotFound);
        }
        Err(e) => return Err(RtfError::Io(e)),
    };

    // Write RTF bytes to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(rtf_bytes)?;
        // stdin is dropped here, closing the pipe
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(RtfError::ConversionFailed(stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
