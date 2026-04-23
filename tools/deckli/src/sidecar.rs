/// Sidecar bridge lifecycle management.
///
/// The CLI auto-starts the Node.js sidecar bridge if it isn't running.
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    #[error("failed to locate deckli-bridge.mjs: {0}")]
    NotFound(String),
    #[error("failed to start sidecar: {0}")]
    StartFailed(String),
    #[error("node.js not found — install Node.js to use deckli")]
    NodeNotFound,
}

/// Returns the path to deckli-bridge.mjs, searching next to the CLI binary.
pub fn bridge_script_path() -> Result<PathBuf, SidecarError> {
    let exe = std::env::current_exe()
        .map_err(|e| SidecarError::NotFound(e.to_string()))?;
    let dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));

    // Look in: same dir, ../bridge/, ../lib/deckli/
    let candidates = [
        dir.join("deckli-bridge.mjs"),
        dir.join("bridge").join("deckli-bridge.mjs"),
        dir.parent()
            .unwrap_or(dir)
            .join("lib")
            .join("deckli")
            .join("deckli-bridge.mjs"),
    ];

    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(SidecarError::NotFound(format!(
        "searched: {}",
        candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

/// Start the sidecar bridge as a detached background process.
pub fn start_bridge() -> Result<(), SidecarError> {
    let script = bridge_script_path()?;

    // Verify node is available
    Command::new("node")
        .arg("--version")
        .output()
        .map_err(|_| SidecarError::NodeNotFound)?;

    Command::new("node")
        .arg(&script)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| SidecarError::StartFailed(e.to_string()))?;

    Ok(())
}

/// Check if a process is listening on the bridge port.
pub fn is_bridge_running(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}
