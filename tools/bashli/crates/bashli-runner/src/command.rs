use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use bashli_core::{ExecError, StderrMode, StdoutMode};

use crate::process_group;
use crate::timeout::with_timeout;

/// Executes shell commands asynchronously with configurable I/O handling,
/// timeout enforcement, and process-group management.
pub struct CommandRunner {
    /// Shell command and arguments, e.g. `["/bin/sh", "-c"]`.
    shell: Vec<String>,
    /// Default timeout applied when `RunOpts::timeout` is `None`.
    default_timeout: Duration,
}

/// Options controlling how a single command invocation behaves.
pub struct RunOpts {
    /// Working directory for the child process.
    pub cwd: Option<PathBuf>,
    /// Additional environment variables for the child process.
    pub env: BTreeMap<String, String>,
    /// How to handle stdout.
    pub stdout_mode: StdoutMode,
    /// How to handle stderr.
    pub stderr_mode: StderrMode,
    /// Data to pipe into the child's stdin.
    pub stdin_data: Option<Vec<u8>>,
    /// Per-invocation timeout override.
    pub timeout: Option<Duration>,
}

/// Raw captured output from a completed command.
#[derive(Debug)]
pub struct RawOutput {
    /// Captured stdout bytes.
    pub stdout: Vec<u8>,
    /// Captured stderr bytes (empty when merged or discarded).
    pub stderr: Vec<u8>,
    /// Process exit code (`-1` if the process was killed by a signal).
    pub exit_code: i32,
    /// Wall-clock duration of the command execution.
    pub duration: Duration,
}

impl Default for RunOpts {
    fn default() -> Self {
        Self {
            cwd: None,
            env: BTreeMap::new(),
            stdout_mode: StdoutMode::default(),
            stderr_mode: StderrMode::default(),
            stdin_data: None,
            timeout: None,
        }
    }
}

impl CommandRunner {
    /// Create a new runner.
    ///
    /// # Arguments
    /// * `shell` — Shell invocation, e.g. `vec!["/bin/sh".into(), "-c".into()]`.
    /// * `default_timeout` — Timeout used when `RunOpts::timeout` is `None`.
    pub fn new(shell: Vec<String>, default_timeout: Duration) -> Self {
        Self {
            shell,
            default_timeout,
        }
    }

    /// Execute `cmd` through the configured shell with the given options.
    ///
    /// On Unix the child is spawned in its own process group so that the
    /// entire tree can be killed on timeout.
    pub async fn run(&self, cmd: &str, opts: &RunOpts) -> Result<RawOutput, ExecError> {
        let timeout_dur = opts.timeout.unwrap_or(self.default_timeout);

        with_timeout(timeout_dur, self.run_inner(cmd, opts, timeout_dur)).await
    }

    /// Inner implementation without the timeout wrapper.
    async fn run_inner(
        &self,
        cmd: &str,
        opts: &RunOpts,
        _timeout_dur: Duration,
    ) -> Result<RawOutput, ExecError> {
        // --- build the Command ------------------------------------------------
        let (program, base_args) = match self.shell.split_first() {
            Some((prog, args)) => (prog.clone(), args.to_vec()),
            None => {
                return Err(ExecError::IoError(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "shell list must not be empty",
                )));
            }
        };

        let mut command = Command::new(&program);
        for arg in &base_args {
            command.arg(arg);
        }
        command.arg(cmd);

        // Working directory
        if let Some(ref cwd) = opts.cwd {
            command.current_dir(cwd);
        }

        // Environment variables
        for (k, v) in &opts.env {
            command.env(k, v);
        }

        // Stdin
        if opts.stdin_data.is_some() {
            command.stdin(Stdio::piped());
        } else {
            command.stdin(Stdio::null());
        }

        // Stdout
        match &opts.stdout_mode {
            StdoutMode::Capture | StdoutMode::Tee { .. } => {
                command.stdout(Stdio::piped());
            }
            StdoutMode::Discard | StdoutMode::File { .. } => {
                command.stdout(Stdio::null());
            }
        }

        // Stderr
        match &opts.stderr_mode {
            StderrMode::Capture => {
                command.stderr(Stdio::piped());
            }
            StderrMode::Merge => {
                // Merge stderr into stdout by redirecting stderr to the stdout pipe.
                // We use `Stdio::piped()` and will merge later after capture.
                command.stderr(Stdio::piped());
            }
            StderrMode::Discard => {
                command.stderr(Stdio::null());
            }
            StderrMode::File { .. } => {
                // For file-based stderr we still capture it and write afterwards.
                command.stderr(Stdio::piped());
            }
        }

        // Unix: spawn in own process group for clean kill
        process_group::spawn_in_own_group(&mut command);

        // --- spawn ------------------------------------------------------------
        let start = Instant::now();
        let mut child = command.spawn()?;

        // Feed stdin if requested
        if let Some(ref data) = opts.stdin_data {
            if let Some(mut stdin_handle) = child.stdin.take() {
                stdin_handle.write_all(data).await?;
                // Drop to close stdin so the child sees EOF.
                drop(stdin_handle);
            }
        }

        // Wait for completion (timeout is enforced by the outer wrapper)
        let output = child.wait_with_output().await?;
        let duration = start.elapsed();

        // --- collect output ---------------------------------------------------
        let exit_code = output.status.code().unwrap_or(-1);

        let (stdout, stderr) = match &opts.stderr_mode {
            StderrMode::Merge => {
                // Combine stderr bytes after stdout bytes.
                let mut merged = output.stdout;
                merged.extend_from_slice(&output.stderr);
                (merged, Vec::new())
            }
            StderrMode::Discard => (output.stdout, Vec::new()),
            StderrMode::Capture => (output.stdout, output.stderr),
            StderrMode::File { path, append } => {
                // Write captured stderr to the specified file.
                use tokio::fs::OpenOptions;
                let mut file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .append(*append)
                    .truncate(!*append)
                    .open(path)
                    .await?;
                tokio::io::AsyncWriteExt::write_all(&mut file, &output.stderr).await?;
                (output.stdout, Vec::new())
            }
        };

        // Handle StdoutMode::Tee and File (write captured stdout to a file)
        match &opts.stdout_mode {
            StdoutMode::Tee { path, append } => {
                use tokio::fs::OpenOptions;
                let mut file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .append(*append)
                    .truncate(!*append)
                    .open(path)
                    .await?;
                tokio::io::AsyncWriteExt::write_all(&mut file, &stdout).await?;
                // Tee: keep stdout in the result as well.
            }
            StdoutMode::File { path, append } => {
                use tokio::fs::OpenOptions;
                let mut file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .append(*append)
                    .truncate(!*append)
                    .open(path)
                    .await?;
                tokio::io::AsyncWriteExt::write_all(&mut file, &stdout).await?;
            }
            _ => {}
        }

        Ok(RawOutput {
            stdout,
            stderr,
            exit_code,
            duration,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_runner() -> CommandRunner {
        CommandRunner::new(
            vec!["/bin/sh".into(), "-c".into()],
            Duration::from_secs(10),
        )
    }

    #[tokio::test]
    async fn run_simple_echo() {
        let runner = default_runner();
        let opts = RunOpts::default();
        let out = runner.run("echo hello", &opts).await.unwrap();
        assert_eq!(out.exit_code, 0);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("hello"));
    }

    #[tokio::test]
    async fn run_captures_exit_code() {
        let runner = default_runner();
        let opts = RunOpts::default();
        let out = runner.run("exit 42", &opts).await.unwrap();
        assert_eq!(out.exit_code, 42);
    }

    #[tokio::test]
    async fn run_with_stdin() {
        let runner = default_runner();
        let opts = RunOpts {
            stdin_data: Some(b"piped input".to_vec()),
            ..RunOpts::default()
        };
        let out = runner.run("cat", &opts).await.unwrap();
        assert_eq!(out.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&out.stdout), "piped input");
    }

    #[tokio::test]
    async fn run_with_env() {
        let runner = default_runner();
        let mut env = BTreeMap::new();
        env.insert("MY_VAR".into(), "my_value".into());
        let opts = RunOpts {
            env,
            ..RunOpts::default()
        };
        let out = runner.run("echo $MY_VAR", &opts).await.unwrap();
        assert_eq!(out.exit_code, 0);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("my_value"));
    }

    #[tokio::test]
    async fn run_stderr_capture() {
        let runner = default_runner();
        let opts = RunOpts {
            stderr_mode: StderrMode::Capture,
            ..RunOpts::default()
        };
        let out = runner.run("echo err >&2", &opts).await.unwrap();
        assert_eq!(out.exit_code, 0);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("err"));
        assert!(out.stdout.is_empty() || String::from_utf8_lossy(&out.stdout).trim().is_empty());
    }

    #[tokio::test]
    async fn run_stderr_merge() {
        let runner = default_runner();
        let opts = RunOpts {
            stderr_mode: StderrMode::Merge,
            ..RunOpts::default()
        };
        let out = runner
            .run("echo out; echo err >&2", &opts)
            .await
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("out"));
        assert!(stdout.contains("err"));
        assert!(out.stderr.is_empty());
    }

    #[tokio::test]
    async fn run_stderr_discard() {
        let runner = default_runner();
        let opts = RunOpts {
            stderr_mode: StderrMode::Discard,
            ..RunOpts::default()
        };
        let out = runner.run("echo err >&2; echo ok", &opts).await.unwrap();
        assert!(out.stderr.is_empty());
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("ok"));
    }

    #[tokio::test]
    async fn run_stdout_discard() {
        let runner = default_runner();
        let opts = RunOpts {
            stdout_mode: StdoutMode::Discard,
            ..RunOpts::default()
        };
        let out = runner.run("echo hello", &opts).await.unwrap();
        assert!(out.stdout.is_empty());
    }

    #[tokio::test]
    async fn run_timeout() {
        let runner = default_runner();
        let opts = RunOpts {
            timeout: Some(Duration::from_millis(100)),
            ..RunOpts::default()
        };
        let result = runner.run("sleep 30", &opts).await;
        match result {
            Err(ExecError::Timeout(_)) => {} // expected
            other => panic!("expected Timeout, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn run_with_cwd() {
        let runner = default_runner();
        let opts = RunOpts {
            cwd: Some(PathBuf::from("/tmp")),
            ..RunOpts::default()
        };
        let out = runner.run("pwd", &opts).await.unwrap();
        assert_eq!(out.exit_code, 0);
        let stdout = String::from_utf8_lossy(&out.stdout);
        // On macOS /tmp is a symlink to /private/tmp
        assert!(stdout.contains("tmp"));
    }

    #[tokio::test]
    async fn run_records_duration() {
        let runner = default_runner();
        let opts = RunOpts::default();
        let out = runner.run("echo fast", &opts).await.unwrap();
        // Duration should be non-zero but very short
        assert!(out.duration.as_secs() < 5);
    }
}
