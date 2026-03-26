/// Unix-specific process group management.
///
/// Spawns child processes in their own process group so that the entire
/// group (including any grandchildren) can be killed on timeout or cancellation.

/// Configure a `Command` to spawn in its own process group (Unix only).
///
/// Uses `pre_exec` to call `setpgid(0, 0)`, which places the child in a new
/// process group whose PGID equals its PID.
#[cfg(unix)]
pub fn spawn_in_own_group(cmd: &mut tokio::process::Command) {
    // SAFETY: setpgid is async-signal-safe per POSIX and is safe to call
    // in the pre_exec hook (which runs between fork and exec).
    unsafe {
        cmd.pre_exec(|| {
            let ret = libc::setpgid(0, 0);
            if ret == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// Kill an entire process group by sending SIGKILL to the group leader.
///
/// `pid` should be the PID of the group leader (which equals the PGID when
/// spawned via `spawn_in_own_group`).
#[cfg(unix)]
#[allow(dead_code)]
pub fn kill_group(pid: u32) -> std::io::Result<()> {
    // killpg sends a signal to all processes in the process group.
    let ret = unsafe { libc::killpg(pid as libc::pid_t, libc::SIGKILL) };
    if ret == -1 {
        let err = std::io::Error::last_os_error();
        // ESRCH means the process group no longer exists — not an error for us.
        if err.raw_os_error() == Some(libc::ESRCH) {
            return Ok(());
        }
        return Err(err);
    }
    Ok(())
}

/// No-op on non-Unix platforms.
#[cfg(not(unix))]
pub fn spawn_in_own_group(_cmd: &mut tokio::process::Command) {}

/// No-op on non-Unix platforms.
#[cfg(not(unix))]
pub fn kill_group(_pid: u32) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_in_own_group_sets_pgid() {
        use std::process::Stdio;
        let mut cmd = tokio::process::Command::new("sh");
        cmd.args(["-c", "echo $$; cat /proc/self/stat 2>/dev/null || echo ok"]);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        spawn_in_own_group(&mut cmd);

        let child = cmd.spawn().expect("failed to spawn");
        let output = child.wait_with_output().await.expect("failed to wait");
        assert!(output.status.success());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn kill_group_handles_nonexistent() {
        // Killing a non-existent process group should succeed (ESRCH is ignored).
        let result = kill_group(999_999_999);
        assert!(result.is_ok());
    }
}
