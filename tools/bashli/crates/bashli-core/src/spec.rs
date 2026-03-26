use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::conditions::{AssertCondition, AssertFailAction};
use crate::extraction::Extraction;
use crate::transform::Transform;

/// Top-level input object for bashli.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Human-readable description (replaces bash comments)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Execution mode for the steps array
    #[serde(default)]
    pub mode: ExecutionMode,

    /// Global settings applied to all steps
    #[serde(default)]
    pub settings: GlobalSettings,

    /// Variable definitions (computed before steps run)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub let_vars: Option<BTreeMap<String, String>>,

    /// The command steps to execute
    pub steps: Vec<Step>,

    /// Which captured variables to include in the final output
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<Vec<String>>,
}

/// Execution mode for the step array.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Stop on first non-zero exit code
    #[default]
    Sequential,
    /// Run all steps regardless of exit codes
    Independent,
    /// Run all steps concurrently
    Parallel,
    /// Run steps concurrently with a max concurrency limit
    ParallelN(usize),
}

/// Global settings applied to all steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    /// How to handle stderr for all steps
    #[serde(default)]
    pub stderr: StderrMode,

    /// How to handle stdout for all steps
    #[serde(default)]
    pub stdout: StdoutMode,

    /// Maximum total output tokens across all steps
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<usize>,

    /// Default timeout per step in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// Working directory for all steps
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,

    /// Environment variables to set
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,

    /// Shell to use
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<Vec<String>>,

    /// Output verbosity
    #[serde(default)]
    pub verbosity: Verbosity,

    /// Token budget configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<TokenBudget>,

    /// Read-only mode — disables write steps
    #[serde(default)]
    pub read_only: bool,

    /// Restrict write targets to these path patterns
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_paths: Option<Vec<String>>,
}

fn default_timeout() -> u64 {
    30_000
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            stderr: StderrMode::default(),
            stdout: StdoutMode::default(),
            max_output_tokens: None,
            timeout_ms: default_timeout(),
            cwd: None,
            env: None,
            shell: None,
            verbosity: Verbosity::default(),
            token_budget: None,
            read_only: false,
            allowed_paths: None,
        }
    }
}

/// Output verbosity level.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verbosity {
    Minimal,
    #[default]
    Normal,
    Full,
    Debug,
}

/// Controls where stderr goes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StderrMode {
    /// Merge stderr into stdout (default)
    #[default]
    Merge,
    /// Discard stderr entirely
    Discard,
    /// Capture stderr separately
    Capture,
    /// Write stderr to a file
    File {
        path: String,
        #[serde(default)]
        append: bool,
    },
}

/// Controls where stdout goes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StdoutMode {
    /// Capture stdout for transforms/variables (default)
    #[default]
    Capture,
    /// Discard stdout entirely
    Discard,
    /// Write stdout to a file AND capture
    Tee {
        path: String,
        #[serde(default)]
        append: bool,
    },
    /// Write stdout to a file ONLY
    File {
        path: String,
        #[serde(default)]
        append: bool,
    },
}

/// A single step in the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Step {
    /// Shorthand: a bare string is treated as a simple command
    BareCmd(String),
    /// A structured step
    Structured(StructuredStep),
}

/// Structured step variants — deserialized by field presence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StructuredStep {
    Cmd(CmdStep),
    Let(LetStep),
    Assert(AssertStep),
    Write(WriteStepWrapper),
    Read(ReadStepWrapper),
    If(IfStep),
    ForEach(ForEachStep),
    Extension(ExtensionStep),
}

/// Execute a shell command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmdStep {
    /// The shell command to execute
    pub cmd: String,

    /// Capture stdout into a named variable
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<String>,

    /// Output transform applied before capture
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<Transform>,

    /// Extract named subvariables via patterns
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extract: Option<BTreeMap<String, Extraction>>,

    /// Step-level stdout override
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<StdoutMode>,

    /// Step-level stderr override
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<StderrMode>,

    /// Pipe a variable's content as stdin
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin: Option<String>,

    /// Step-level timeout override
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    /// Step-level cwd override
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,

    /// Step-level environment variables
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,

    /// Max output lines/bytes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<LimitSpec>,

    /// Retry on failure
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetrySpec>,

    /// Step to execute on failure
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<Box<Step>>,

    /// Whether to include full output in the response
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
}

/// Set/compute variables without running a command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LetStep {
    #[serde(rename = "let")]
    pub bindings: BTreeMap<String, String>,
}

/// Conditional assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertStep {
    /// Variable to check
    #[serde(rename = "assert")]
    pub var: String,

    /// Condition to assert
    #[serde(flatten)]
    pub condition: AssertCondition,

    /// Human-readable failure message
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// What to do on assertion failure
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_fail: Option<AssertFailAction>,
}

/// Wrapper for write step (uses `write` key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteStepWrapper {
    pub write: WriteStep,
}

/// Write to a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteStep {
    /// Output file path
    pub path: String,
    /// Content to write
    pub content: String,
    /// Write mode
    #[serde(default)]
    pub mode: WriteMode,
    /// Create parent directories if they don't exist
    #[serde(default)]
    pub mkdir: bool,
}

/// Write mode for file operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WriteMode {
    /// Overwrite existing file
    #[default]
    Create,
    /// Append to file
    Append,
    /// Atomic write (temp file then rename)
    Atomic,
    /// Only write if file doesn't exist
    CreateNew,
}

/// Wrapper for read step (uses `read` key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadStepWrapper {
    pub read: ReadStep,
}

/// Read a file into a variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadStep {
    /// File path to read
    pub path: String,
    /// Capture contents into a variable
    pub capture: String,
    /// Transform to apply after reading
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<Transform>,
    /// Limit specification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<LimitSpec>,
}

/// If/Else branching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfStep {
    /// Condition to evaluate (contains var and condition)
    #[serde(rename = "if")]
    pub condition: IfCondition,
    /// Steps to run if true
    pub then: Vec<Step>,
    /// Steps to run if false
    #[serde(default, rename = "else", skip_serializing_if = "Option::is_none")]
    pub else_steps: Option<Vec<Step>>,
}

/// Condition for an if step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfCondition {
    pub var: String,
    #[serde(flatten)]
    pub condition: AssertCondition,
}

/// ForEach iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForEachStep {
    /// Variable containing the iterable
    pub for_each: String,
    /// Loop variable name
    #[serde(rename = "as")]
    pub as_var: String,
    /// Steps to execute per iteration
    pub steps: Vec<Step>,
    /// How to collect results
    #[serde(default)]
    pub collect: CollectMode,
    /// Maximum concurrent iterations
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<usize>,
    /// Capture collected results into a variable
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<String>,
}

/// How to collect ForEach results.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CollectMode {
    #[default]
    Array,
    Map,
    Filter,
    Concat,
    Discard,
}

/// Extension step — dispatched via StepRegistry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionStep {
    pub extension: ExtensionStepInner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionStepInner {
    pub kind: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Limit specification for output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitSpec {
    /// Maximum number of output lines
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<usize>,
    /// Maximum number of output bytes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<usize>,
    /// Truncation strategy
    #[serde(default)]
    pub strategy: TruncationStrategy,
}

/// Where to truncate.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TruncationStrategy {
    /// Keep first N lines
    #[default]
    Head,
    /// Keep last N lines
    Tail,
    /// Keep first N/2 and last N/2 with gap marker
    Smart,
    /// Keep lines matching a regex
    Filter(String),
}

/// Retry specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrySpec {
    /// Maximum number of attempts (including the first)
    #[serde(default = "default_max_attempts")]
    pub max_attempts: usize,
    /// Delay between attempts in milliseconds
    #[serde(default = "default_backoff_ms")]
    pub backoff_ms: u64,
    /// Multiply backoff by this factor after each retry
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// Maximum backoff in milliseconds
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_backoff_ms: Option<u64>,
    /// Only retry on specific exit codes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_on_exit_codes: Option<Vec<i32>>,
}

fn default_max_attempts() -> usize { 3 }
fn default_backoff_ms() -> u64 { 1000 }
fn default_backoff_multiplier() -> f64 { 2.0 }

/// Token budget configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    /// Maximum approximate token count for all step outputs combined
    pub max_tokens: usize,
    /// How to allocate budget across steps
    #[serde(default)]
    pub allocation: BudgetAllocation,
    /// What to do when budget is exhausted
    #[serde(default)]
    pub overflow: OverflowStrategy,
}

/// How to allocate budget across steps.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetAllocation {
    #[default]
    Equal,
    BackWeighted,
    FrontWeighted,
    Weighted(Vec<f64>),
}

/// What to do when budget is exhausted.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverflowStrategy {
    #[default]
    Truncate,
    MetadataOnly,
    Abort,
}
