# bashli — Structured Bash Execution Engine for AI Agents

> **Version:** 1.0.0-draft  
> **Author:** Brian  
> **Date:** 2026-03-22  
> **Language:** Rust  
> **License:** MIT  

---

## 1. Executive Summary

**bashli** is a Rust-native CLI that accepts a single JSON (or YAML) command string, executes a structured pipeline of shell commands, and returns machine-readable JSON results. It replaces raw bash invocations in agentic workflows — eliminating parser-triggered safety prompts (newlines, comments, redirects, quotes) while adding capabilities that bash lacks: named variable capture, structured output transforms, parallel execution, token-budget-aware truncation, conditional logic, iteration, assertions, and built-in file operations.

bashli is designed as the **universal execution substrate** for AI coding agents. Any agent that today calls `Bash(...)` can instead call `bashli '{...}'` — a single, clean, non-triggering command — and get back structured JSON instead of raw terminal output.

### Design Principles

1. **Single-string invocation.** The entire execution plan is one JSON argument. No pipes, no redirects, no newlines, no comments in the shell layer.
2. **JSON-in, JSON-out.** Agents never parse raw terminal output. Every result is structured.
3. **Bash as a read-only substrate.** bashli executes commands via the system shell but handles file writes, output transforms, and control flow in Rust.
4. **Built-in > piped.** Operations commonly piped through `head`, `tail`, `wc`, `jq`, `grep`, `sort`, `uniq` are native bashli features — no subprocess overhead, no escaping issues.
5. **Token-budget awareness.** Output is shaped for LLM context windows, not human terminals.
6. **Zero-dependency host.** Ships as a single static binary. No runtime dependencies beyond a POSIX shell.
7. **Extensible by default.** Every behavioral surface — step types, transforms, extraction methods — is behind a trait. New capabilities are added by implementing a trait in a new module, not by modifying existing code.

---

## 2. Feature Specification

### 2.1 Input Format

bashli accepts input via CLI argument or stdin:

```bash
# Inline JSON argument
bashli '{"steps":["ls -la","pwd"]}'

# Stdin (for large specs)
cat task.json | bashli -

# YAML input
bashli --yaml task.yaml

# File input
bashli -f task.json
```

### 2.2 Schema: TaskSpec

The top-level input object:

```rust
struct TaskSpec {
    /// Human-readable description (replaces bash comments)
    description: Option<String>,

    /// Execution mode for the steps array
    mode: ExecutionMode,           // default: Sequential

    /// Global settings applied to all steps
    settings: GlobalSettings,

    /// Variable definitions (computed before steps run)
    let_vars: Option<BTreeMap<String, String>>,

    /// The command steps to execute
    steps: Vec<Step>,

    /// Which captured variables to include in the final output
    summary: Option<Vec<String>>,
}

enum ExecutionMode {
    /// Stop on first non-zero exit code (like `&&`)
    Sequential,
    /// Run all steps regardless of exit codes
    Independent,
    /// Run all steps concurrently (read-only commands only)
    Parallel,
    /// Run steps concurrently with a max concurrency limit
    ParallelN(usize),
}

struct GlobalSettings {
    /// How to handle stderr for all steps (default: merge)
    stderr: StderrMode,              // default: Merge
    /// How to handle stdout for all steps (default: capture)
    stdout: StdoutMode,              // default: Capture
    /// Maximum total output tokens across all steps
    max_output_tokens: Option<usize>,
    /// Default timeout per step in milliseconds
    timeout_ms: Option<u64>,           // default: 30_000
    /// Working directory for all steps
    cwd: Option<String>,
    /// Environment variables to set
    env: Option<BTreeMap<String, String>>,
    /// Shell to use (default: /bin/sh -c)
    shell: Option<Vec<String>>,
}

/// Controls where stderr goes. Replaces all `2>` redirect patterns.
enum StderrMode {
    /// Merge stderr into stdout (replaces `2>&1`). DEFAULT.
    Merge,
    /// Discard stderr entirely (replaces `2>/dev/null`)
    Discard,
    /// Capture stderr separately in StepResult.stderr
    Capture,
    /// Write stderr to a file (replaces `2> file` / `2>> file`)
    File { path: String, append: bool },
}

/// Controls where stdout goes. Replaces all `>` redirect patterns.
enum StdoutMode {
    /// Capture stdout for transforms/variables. DEFAULT.
    Capture,
    /// Discard stdout entirely (replaces `> /dev/null`)
    Discard,
    /// Write stdout to a file AND capture (replaces `| tee file`)
    Tee { path: String, append: bool },
    /// Write stdout to a file ONLY (replaces `> file` / `>> file`)
    File { path: String, append: bool },
}
```

### 2.3 Schema: Step

Each step in the pipeline is a tagged enum. The enum is **open for extension** — new variants are added by implementing the `StepExecutor` trait (see §4.5) and registering with the `StepRegistry`:

```rust
enum Step {
    /// Execute a shell command
    Cmd(CmdStep),
    /// Set/compute variables without running a command
    Let(BTreeMap<String, String>),
    /// Conditional assertion
    Assert(AssertStep),
    /// Iterate over a captured variable
    ForEach(ForEachStep),
    /// Write to a file (no shell redirect needed)
    Write(WriteStep),
    /// Read a file into a variable
    Read(ReadStep),
    /// Branch based on a condition
    If(IfStep),
    /// Extension point: any step registered via plugin
    Extension(ExtensionStep),
}

/// A step type contributed by a plugin. The engine does not
/// need to know the concrete type — it dispatches via the
/// StepRegistry using the `kind` string.
struct ExtensionStep {
    /// Step type identifier, matched against StepRegistry
    kind: String,
    /// Opaque JSON config passed to the StepExecutor
    config: serde_json::Value,
}

struct CmdStep {
    /// The shell command to execute.
    /// MUST NOT contain redirect operators (>, >>, 2>, 2>&1, &>, |).
    /// Use the `stdout`, `stderr`, and `stdin` fields instead.
    cmd: String,

    /// Capture stdout into a named variable ($VAR_NAME)
    capture: Option<String>,

    /// Output transform applied before capture
    transform: Option<Transform>,

    /// Extract named subvariables via patterns
    extract: Option<BTreeMap<String, Extraction>>,

    /// Step-level overrides for I/O routing
    /// (overrides GlobalSettings for this step only)
    stdout: Option<StdoutMode>,
    stderr: Option<StderrMode>,

    /// Pipe a variable's content as stdin to the command
    /// (replaces `echo $VAR | cmd` and `cmd <<< $VAR` and `cmd < file`)
    stdin: Option<String>,

    /// Step-level overrides
    timeout_ms: Option<u64>,
    cwd: Option<String>,
    env: Option<BTreeMap<String, String>>,

    /// Max output lines (replaces `| head -N`)
    limit: Option<LimitSpec>,

    /// Retry on failure
    retry: Option<RetrySpec>,

    /// Step to execute on failure
    on_failure: Option<Box<Step>>,

    /// Whether to include full output in the response
    /// (false = output available via variable only)
    verbose: Option<bool>,
}
```

### 2.4 Variable System

Variables are the core mechanism for inter-step communication.

#### Naming Convention

- All user variables are prefixed with `$` in definitions and interpolations
- System variables are prefixed with `$_` (read-only)
- Environment variables accessed via `$ENV.NAME`

#### System Variables (auto-populated)

| Variable | Description |
|---|---|
| `$_CWD` | Current working directory |
| `$_HOME` | User home directory |
| `$_OS` | Operating system (linux, macos) |
| `$_ARCH` | Architecture (x86_64, aarch64) |
| `$_TIMESTAMP` | ISO 8601 timestamp at execution start |
| `$_STEP_INDEX` | Current step index (0-based) |
| `$_PREV_EXIT` | Previous step's exit code |
| `$_PREV_STDOUT` | Previous step's stdout (truncated to 4KB) |

#### Variable Capture

```json
{
  "cmd": "cargo metadata --format-version 1",
  "capture": "$META",
  "transform": "json"
}
```

After execution, `$META` contains the parsed JSON. Later steps can interpolate:

```json
{
  "cmd": "ls $META.workspace_root/src/"
}
```

#### Variable Interpolation Rules

1. `$VAR` — string interpolation into command
2. `$VAR.field` — JSON path access (dot notation)
3. `$VAR[0]` — array index access
4. `$VAR.field[2].name` — nested access
5. `${VAR}` — explicit boundary (e.g., `${VAR}_suffix`)
6. `$$` — literal `$` escape

Interpolation happens in: `cmd`, `write.content`, `write.path`, `assert.value`, `assert.equals`, `let` values, `cwd`, and `env` values.

#### Let Bindings

Pre-compute variables from other variables or literals:

```json
{
  "let": {
    "$SRC": "$META.workspace_root/src",
    "$TARGET": "$META.target_directory",
    "$CRATE": "$META.resolve.root"
  }
}
```

#### Variable Scoping

- `capture` variables: global within the TaskSpec invocation
- `for_each` loop variables: scoped to the loop body
- `let` variables: global from point of definition onward
- System variables: always available, read-only
- Variables are immutable once set (a second `capture` to `$X` overwrites)

### 2.5 Built-in Transforms

Transforms replace common piped commands with Rust-native implementations. Applied to stdout before capture. Every transform implements the `TransformFn` trait (see §4.6), making the system open for extension.

```rust
enum Transform {
    /// Raw string, no transformation (default)
    Raw,
    /// Trim leading/trailing whitespace
    Trim,
    /// Split into JSON array of lines (replaces manual line parsing)
    Lines,
    /// Parse stdout as JSON (replaces `| jq .`)
    Json,
    /// Count lines (replaces `| wc -l`)
    CountLines,
    /// Count bytes (replaces `| wc -c`)
    CountBytes,
    /// Count words (replaces `| wc -w`)
    CountWords,
    /// First N lines (replaces `| head -N`)
    Head(usize),
    /// Last N lines (replaces `| tail -N`)
    Tail(usize),
    /// Sort lines (replaces `| sort`)
    Sort(SortSpec),
    /// Unique lines (replaces `| sort -u`)
    Unique,
    /// Apply a jaq filter expression (replaces `| jq 'EXPR'`)
    Jq(String),
    /// Apply sed commands (replaces `| sed 'EXPR'`) — via sedregex crate
    Sed(String),
    /// Apply an awk program (replaces `| awk 'PROGRAM'`) — via awk-rs crate
    Awk(AwkSpec),
    /// Regex filter — keep matching lines (replaces `| grep PATTERN`)
    Grep(GrepSpec),
    /// Split on a delimiter into JSON array (replaces `| cut -d',' -f1`)
    Split(String),
    /// Chain multiple transforms
    Pipe(Vec<Transform>),
    /// Base64 encode
    Base64Encode,
    /// Base64 decode
    Base64Decode,
    /// Format as a markdown code block
    CodeBlock(Option<String>),
    /// Extract with a regex, returning captured groups
    Regex(String),
    /// Compute a SHA-256 hash of the output
    Sha256,
    /// Extension point: any transform registered via plugin
    Extension { name: String, config: serde_json::Value },
}

struct SortSpec {
    /// Sort numerically instead of lexicographically
    numeric: bool,
    /// Reverse sort order
    reverse: bool,
    /// Sort by field (for JSON array of objects, via jaq)
    by: Option<String>,
}

struct GrepSpec {
    /// Regex pattern to match
    pattern: String,
    /// Invert match (replaces `grep -v`)
    invert: bool,
    /// Case insensitive (replaces `grep -i`)
    ignore_case: bool,
    /// Return only matching portion (replaces `grep -o`)
    only_matching: bool,
    /// Count matches instead of returning lines (replaces `grep -c`)
    count: bool,
}

struct AwkSpec {
    /// The awk program string (e.g., "{print $2}")
    program: String,
    /// Field separator (replaces `awk -F`)
    field_separator: Option<String>,
    /// Variables to set before execution (replaces `awk -v`)
    vars: Option<BTreeMap<String, String>>,
}
```

#### Transform Chaining (Pipe)

Transforms can be chained, replacing multi-pipe bash sequences:

```json
{
  "cmd": "cargo check",
  "capture": "$ERRORS",
  "transform": {
    "pipe": [
      {"grep": {"pattern": "^error\\[", "ignore_case": false}},
      "unique",
      {"sort": {"numeric": false, "reverse": false}},
      {"head": 5}
    ]
  }
}
```

This replaces: `cargo check 2>&1 | grep '^error\[' | sort -u | head -5`

### 2.6 Extraction System

Extract named sub-variables from a step's output using patterns, regex, or jaq. Each extraction method implements the `ExtractorFn` trait (see §4.7):

```rust
struct Extraction {
    /// Extraction method
    method: ExtractionMethod,
}

enum ExtractionMethod {
    /// Apply a jaq filter expression
    Jq(String),
    /// Regex with named capture groups
    Regex(String),
    /// Count lines matching a pattern
    CountMatching(String),
    /// First line matching a pattern
    FirstMatching(String),
    /// All lines matching a pattern (as JSON array)
    AllMatching(String),
    /// Specific line number (0-indexed)
    Line(usize),
    /// A range of lines [start, end) 
    LineRange(usize, usize),
    /// Extension point
    Extension { name: String, config: serde_json::Value },
}
```

Example:

```json
{
  "cmd": "cargo check",
  "capture": "$CHECK_OUTPUT",
  "extract": {
    "$ERROR_COUNT": {"count_matching": "^error"},
    "$WARNING_COUNT": {"count_matching": "^warning"},
    "$FIRST_ERROR": {"first_matching": "^error"},
    "$ERROR_CODES": {
      "regex": "error\\[(E\\d+)\\]",
      "comment": "Extracts all error codes like E0432"
    }
  }
}
```

### 2.7 Limit Specification

Replaces `| head`, `| tail`, and smart truncation for token budgets:

```rust
struct LimitSpec {
    /// Maximum number of output lines
    max_lines: Option<usize>,
    /// Maximum number of output bytes
    max_bytes: Option<usize>,
    /// Where to truncate: head, tail, or smart (head + tail with gap marker)
    strategy: TruncationStrategy,
}

enum TruncationStrategy {
    /// Keep first N lines (like `head`)
    Head,
    /// Keep last N lines (like `tail`)
    Tail,
    /// Keep first N/2 and last N/2, with a marker in between
    Smart,
    /// Keep lines matching a regex, drop others
    Filter(String),
}
```

Example:

```json
{
  "cmd": "find . -name '*.rs' -type f",
  "capture": "$RS_FILES",
  "limit": {"max_lines": 50, "strategy": "smart"}
}
```

Smart output:

```
src/main.rs
src/lib.rs
src/config.rs
... [truncated 142 lines] ...
tests/integration_test.rs
tests/e2e.rs
```

### 2.8 Conditional Logic

#### Assertions

Assert checks a variable against a condition. Used for agent self-verification:

```rust
struct AssertStep {
    /// Variable to check
    var: String,
    /// Condition to assert
    condition: AssertCondition,
    /// Human-readable failure message (supports interpolation)
    message: Option<String>,
    /// What to do on assertion failure
    on_fail: AssertFailAction,
}

enum AssertCondition {
    /// Variable equals a literal value
    Equals(String),
    /// Variable does not equal a literal value
    NotEquals(String),
    /// Variable contains a substring
    Contains(String),
    /// Variable does not contain a substring
    NotContains(String),
    /// Variable matches a regex
    Matches(String),
    /// Variable is empty or unset
    IsEmpty,
    /// Variable is not empty
    IsNotEmpty,
    /// Numeric comparison: variable > value
    GreaterThan(f64),
    /// Numeric comparison: variable < value
    LessThan(f64),
    /// Variable is valid JSON
    IsJson,
    /// Variable (as number) is within range [low, high]
    InRange(f64, f64),
}

enum AssertFailAction {
    /// Stop execution, return error
    Abort,
    /// Skip remaining steps, return success with warning
    SkipRest,
    /// Continue execution, log warning
    Warn,
    /// Run a fallback step
    Fallback(Box<Step>),
}
```

Example:

```json
{
  "assert": "$ERROR_COUNT",
  "equals": "0",
  "message": "Compilation failed with $ERROR_COUNT errors",
  "on_fail": "abort"
}
```

#### If/Else Branching

```rust
struct IfStep {
    /// Condition to evaluate
    condition: AssertCondition,
    /// Variable to test
    var: String,
    /// Steps to run if true
    then: Vec<Step>,
    /// Steps to run if false
    else_steps: Option<Vec<Step>>,
}
```

Example:

```json
{
  "if": {"var": "$ERROR_COUNT", "greater_than": 0},
  "then": [
    {"cmd": "cargo check", "capture": "$DETAIL", "transform": {"tail": 20}}
  ],
  "else": [
    {"cmd": "cargo build --release", "capture": "$BUILD"}
  ]
}
```

### 2.9 Iteration

```rust
struct ForEachStep {
    /// Variable containing the iterable (JSON array or newline-delimited string)
    over: String,
    /// Loop variable name
    as_var: String,
    /// Steps to execute per iteration
    steps: Vec<Step>,
    /// How to collect results
    collect: CollectMode,
    /// Maximum concurrent iterations (for parallel for_each)
    concurrency: Option<usize>,
    /// Capture the collected results into a variable
    capture: Option<String>,
}

enum CollectMode {
    /// JSON array of all outputs
    Array,
    /// JSON object mapping input -> output
    Map,
    /// Only keep items where step exited 0
    Filter,
    /// Concatenate all outputs
    Concat,
    /// Discard outputs (side-effect only)
    Discard,
}
```

Example:

```json
{
  "for_each": "$RS_FILES",
  "as": "$FILE",
  "steps": [
    {"cmd": "grep -c 'TODO\\|FIXME\\|HACK' $FILE || true", "capture": "$COUNT", "transform": "trim"}
  ],
  "collect": "map",
  "capture": "$TODO_MAP"
}
```

Result in `$TODO_MAP`:

```json
{"src/main.rs": "3", "src/lib.rs": "0", "src/config.rs": "7"}
```

### 2.10 File Operations

Built-in file operations replace shell redirects:

```rust
struct WriteStep {
    /// Output file path (supports variable interpolation)
    path: String,
    /// Content to write (supports variable interpolation)
    content: String,
    /// Write mode
    mode: WriteMode,
    /// Create parent directories if they don't exist
    mkdir: bool,
}

enum WriteMode {
    /// Overwrite existing file (replaces `>`)
    Create,
    /// Append to file (replaces `>>`)
    Append,
    /// Write to a temp file, then atomically rename
    Atomic,
    /// Only write if file doesn't exist
    CreateNew,
}

struct ReadStep {
    /// File path to read
    path: String,
    /// Capture contents into a variable
    capture: String,
    /// Transform to apply after reading
    transform: Option<Transform>,
    /// Limit specification
    limit: Option<LimitSpec>,
}
```

Example:

```json
{
  "write": {
    "path": "$WORKSPACE/src/filter.rs",
    "content": "use jaq_core::ValR;\n\npub fn apply_filter(input: &str, expr: &str) -> ValR {\n    todo!()\n}\n",
    "mode": "create_new",
    "mkdir": true
  }
}
```

### 2.11 I/O Routing (Redirect Replacement)

bashli **forbids** redirect operators inside `cmd` strings. The characters `>`, `>>`, `2>`, `2>&1`, `&>`, and `<` are all handled by dedicated JSON fields on `CmdStep` and `GlobalSettings`. This eliminates an entire class of agent safety prompts.

**Validation rule:** During TaskSpec validation, bashli scans every `cmd` string for redirect-like patterns. If found, validation fails with a clear error message directing the user to the correct JSON field. This is enforced in `bashli-core/src/validation.rs`.

#### Complete Redirect Replacement Map

| Bash Redirect | bashli JSON Field | Scope |
|---|---|---|
| `cmd 2>&1` | `"stderr": "merge"` | Per-step or global (this is the default) |
| `cmd 2>/dev/null` | `"stderr": "discard"` | Per-step or global |
| `cmd 2> err.log` | `"stderr": {"file": {"path": "err.log"}}` | Per-step |
| `cmd 2>> err.log` | `"stderr": {"file": {"path": "err.log", "append": true}}` | Per-step |
| `cmd > out.txt` | `"stdout": {"file": {"path": "out.txt"}}` | Per-step |
| `cmd >> out.txt` | `"stdout": {"file": {"path": "out.txt", "append": true}}` | Per-step |
| `cmd > /dev/null` | `"stdout": "discard"` | Per-step or global |
| `cmd > /dev/null 2>&1` | `"stdout": "discard", "stderr": "discard"` | Per-step or global |
| `cmd &> file` | `"stdout": {"file": {"path": "file"}}, "stderr": "merge"` | Per-step |
| `cmd \| tee file` | `"stdout": {"tee": {"path": "file"}}` | Per-step |
| `cmd \| tee -a file` | `"stdout": {"tee": {"path": "file", "append": true}}` | Per-step |
| `echo $VAR \| cmd` | `"stdin": "$VAR"` | Per-step |
| `cmd < file` | Use a `read` step first, then `"stdin": "$FILE_CONTENT"` | Two steps |
| `cmd <<< "text"` | `"stdin": "literal text"` | Per-step |
| `cmd << EOF...EOF` | `"stdin": "$HEREDOC_VAR"` (set via `let` first) | Two steps |

#### Examples

**Before (triggers safety prompt):**
```bash
cargo check 2>&1 | tail -10
```

**After (no prompt):**
```json
{
  "cmd": "cargo check",
  "stderr": "merge",
  "transform": {"tail": 10}
}
```

**Before (triggers safety prompt):**
```bash
echo 'mod filter;' >> /dev/null && cargo check 2>&1 | tail -10
```

**After (clean, no prompt, no pointless echo):**
```json
{
  "steps": [
    {"cmd": "cargo check", "stderr": "merge", "transform": {"tail": 10}, "capture": "$CHECK"}
  ]
}
```

**Before (triggers safety prompt):**
```bash
mycommand > output.log 2> errors.log
```

**After:**
```json
{
  "cmd": "mycommand",
  "stdout": {"file": {"path": "output.log"}},
  "stderr": {"file": {"path": "errors.log"}}
}
```

**Before (triggers safety prompt):**
```bash
cat data.csv | python3 analyze.py > results.json
```

**After:**
```json
{
  "steps": [
    {"read": {"path": "data.csv", "capture": "$DATA"}},
    {
      "cmd": "python3 analyze.py",
      "stdin": "$DATA",
      "stdout": {"file": {"path": "results.json"}},
      "capture": "$RESULTS"
    }
  ]
}
```

#### Default Behavior

When no `stdout` or `stderr` field is specified on a step, the global defaults from `GlobalSettings` apply:

- **`stderr`**: `"merge"` — stderr is merged into stdout. This is the safe default because agents almost always want to see errors alongside output.
- **`stdout`**: `"capture"` — stdout is captured for transforms, extraction, and variable capture.

This means a minimal step like `{"cmd": "cargo check"}` automatically gets `stderr: merge` + `stdout: capture` — the equivalent of `cargo check 2>&1` without any redirect syntax.

### 2.12 Retry Logic

```rust
struct RetrySpec {
    /// Maximum number of attempts (including the first)
    max_attempts: usize,
    /// Delay between attempts in milliseconds
    backoff_ms: u64,
    /// Multiply backoff by this factor after each retry
    backoff_multiplier: f64,
    /// Maximum backoff in milliseconds
    max_backoff_ms: Option<u64>,
    /// Only retry on specific exit codes
    retry_on_exit_codes: Option<Vec<i32>>,
}
```

### 2.13 Token Budget Management

A global system for managing total output size across all steps:

```rust
struct TokenBudget {
    /// Maximum approximate token count for all step outputs combined
    max_tokens: usize,
    /// How to allocate budget across steps
    allocation: BudgetAllocation,
    /// What to do when budget is exhausted
    overflow: OverflowStrategy,
}

enum BudgetAllocation {
    /// Equal share per step
    Equal,
    /// Later steps get priority (useful for build/check results)
    BackWeighted,
    /// Earlier steps get priority (useful for exploration)
    FrontWeighted,
    /// Custom per-step weights
    Weighted(Vec<f64>),
}

enum OverflowStrategy {
    /// Truncate with smart strategy
    Truncate,
    /// Drop entire step output, keep metadata only
    MetadataOnly,
    /// Abort remaining steps
    Abort,
}
```

---

## 3. Output Specification

### 3.1 Response Schema

Every bashli invocation returns:

```rust
struct TaskResult {
    /// Overall success (all steps passed / assertions held)
    ok: bool,
    /// Total wall-clock duration in milliseconds
    duration_ms: u64,
    /// Captured variables (filtered by `summary` if specified)
    variables: BTreeMap<String, serde_json::Value>,
    /// Per-step results
    steps: Vec<StepResult>,
    /// Warnings and non-fatal issues
    warnings: Vec<String>,
    /// If ok is false, a structured error
    error: Option<TaskError>,
}

struct StepResult {
    /// Step index (0-based)
    index: usize,
    /// What kind of step this was
    kind: StepKind,
    /// Exit code (for Cmd steps)
    exit_code: Option<i32>,
    /// Wall-clock duration in milliseconds
    duration_ms: u64,
    /// Stdout (subject to token budget / limit)
    stdout: Option<String>,
    /// Stderr (if not merged)
    stderr: Option<String>,
    /// Whether output was truncated
    truncated: bool,
    /// Lines truncated
    truncated_lines: Option<usize>,
    /// Notes (e.g., "wrote src/filter.rs", "assertion passed")
    note: Option<String>,
    /// Variables captured by this step
    captured: Option<Vec<String>>,
}

enum StepKind {
    Cmd,
    Let,
    Assert,
    ForEach,
    Write,
    Read,
    If,
    Extension(String),
}

struct TaskError {
    /// Step index where failure occurred
    step_index: usize,
    /// Error category
    kind: ErrorKind,
    /// Human-readable message (with interpolated variables)
    message: String,
}

enum ErrorKind {
    /// Command returned non-zero exit code
    NonZeroExit(i32),
    /// Assertion failed
    AssertionFailed,
    /// Command timed out
    Timeout,
    /// Variable not found during interpolation
    UndefinedVariable(String),
    /// JSON parse error in transform
    ParseError,
    /// File operation failed
    IoError,
    /// Invalid TaskSpec
    ValidationError,
    /// Extension step error
    ExtensionError(String),
}
```

### 3.2 Summary Mode

When `summary` is specified, only the listed variables appear in the `variables` field. Step outputs are compressed to metadata only. This is the primary mechanism for token-budget control — the agent requests only the data it needs.

```json
{
  "summary": ["$ERROR_COUNT", "$WARNING_COUNT", "$WORKSPACE"]
}
```

Response:

```json
{
  "ok": true,
  "duration_ms": 1240,
  "variables": {
    "$ERROR_COUNT": "0",
    "$WARNING_COUNT": "3",
    "$WORKSPACE": "/Users/brian/projects/conf-cli"
  },
  "steps": [
    {"index": 0, "kind": "cmd", "exit_code": 0, "duration_ms": 380},
    {"index": 1, "kind": "cmd", "exit_code": 0, "duration_ms": 12},
    {"index": 2, "kind": "assert", "duration_ms": 0, "note": "passed"},
    {"index": 3, "kind": "cmd", "exit_code": 0, "duration_ms": 840}
  ]
}
```

### 3.3 Verbosity Levels

Controlled via `--verbosity` flag or `settings.verbosity`:

| Level | Behavior |
|---|---|
| `minimal` | Only `ok`, `duration_ms`, `variables`, `error`. Steps array omitted. |
| `normal` (default) | Steps include metadata + captured vars. Stdout only if `verbose: true` on step. |
| `full` | All stdout/stderr included for every step. |
| `debug` | Includes interpolated commands, timing breakdown, variable resolution trace. |

---

## 4. Architectural Specification

### 4.1 Design Philosophy: Trait-Oriented Extensibility

bashli follows the **Open-Closed Principle** at every behavioral boundary. The system is extended by *implementing traits and registering* — never by modifying existing match arms or adding to monolithic enums.

Three extension axes:

| Axis | Trait | Registration | Example Extension |
|---|---|---|---|
| New step types | `StepExecutor` | `StepRegistry::register()` | `HttpStep`, `SqlStep`, `GitStep` |
| New transforms | `TransformFn` | `TransformRegistry::register()` | `Csv`, `Toml`, `XmlPath` |
| New extractors | `ExtractorFn` | `ExtractorRegistry::register()` | `JsonSchema`, `Semver` |

Each registry is built at startup from compiled-in defaults plus optional plugin contributions.

### 4.2 Crate Structure (Cargo Workspace)

```
bashli/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── bashli-core/              # Pure types — zero logic, zero I/O
│   │   ├── Cargo.toml            #   deps: serde, serde_json
│   │   └── src/
│   │       ├── lib.rs            #   Re-exports
│   │       ├── spec.rs           #   TaskSpec, Step, CmdStep, all input types
│   │       ├── result.rs         #   TaskResult, StepResult, all output types
│   │       ├── transform.rs      #   Transform enum (data only, no impl)
│   │       ├── extraction.rs     #   Extraction enum (data only, no impl)
│   │       ├── conditions.rs     #   AssertCondition, AssertFailAction
│   │       └── error.rs          #   All error types (VarError, ExecError, etc.)
│   │
│   ├── bashli-vars/              # Variable store + interpolation engine
│   │   ├── Cargo.toml            #   deps: bashli-core, serde_json, regex
│   │   └── src/
│   │       ├── lib.rs            #   Re-exports
│   │       ├── store.rs          #   VarStore: globals, scope stack, get/set
│   │       ├── interpolate.rs    #   Template interpolation ($VAR, $VAR.field[0])
│   │       ├── path.rs           #   JSON path resolution (dot/bracket notation)
│   │       └── escape.rs         #   Shell-safe escaping for interpolated values
│   │
│   ├── bashli-transforms/        # Transform trait + all built-in implementations
│   │   ├── Cargo.toml            #   deps: bashli-core, bashli-jq, bashli-sed, bashli-awk,
│   │   │                         #         regex, sha2, base64
│   │   └── src/
│   │       ├── lib.rs            #   TransformFn trait + TransformRegistry
│   │       ├── registry.rs       #   TransformRegistry: register, resolve, default_registry()
│   │       ├── text.rs           #   Trim, Lines, CountLines, CountBytes, CountWords
│   │       ├── slice.rs          #   Head, Tail, limit/truncation logic
│   │       ├── sort.rs           #   Sort, Unique
│   │       ├── grep.rs           #   Grep (all modes: invert, count, only-matching)
│   │       ├── json.rs           #   Json parse, Split
│   │       ├── jq.rs             #   Jq transform (delegates to bashli-jq)
│   │       ├── sed.rs            #   Sed transform (delegates to bashli-sed)
│   │       ├── awk.rs            #   Awk transform (delegates to bashli-awk)
│   │       ├── encode.rs         #   Base64Encode, Base64Decode, Sha256
│   │       ├── format.rs         #   CodeBlock, Regex capture
│   │       └── pipe.rs           #   Pipe chaining: applies transforms sequentially
│   │
│   ├── bashli-extract/           # Extraction trait + all built-in implementations
│   │   ├── Cargo.toml            #   deps: bashli-core, bashli-jq, regex
│   │   └── src/
│   │       ├── lib.rs            #   ExtractorFn trait + ExtractorRegistry
│   │       ├── registry.rs       #   ExtractorRegistry: register, resolve, default_registry()
│   │       ├── pattern.rs        #   CountMatching, FirstMatching, AllMatching
│   │       ├── lines.rs          #   Line, LineRange
│   │       ├── regex.rs          #   Regex with named capture groups
│   │       └── jq.rs             #   Jq extraction (delegates to bashli-jq)
│   │
│   ├── bashli-jq/                # jaq integration — thin wrapper
│   │   ├── Cargo.toml            #   deps: jaq-core, jaq-parse, jaq-std, serde_json
│   │   └── src/
│   │       ├── lib.rs            #   Public API: compile(), eval(), eval_to_string()
│   │       ├── compiler.rs       #   jaq filter compilation + caching (LRU)
│   │       └── eval.rs           #   Filter evaluation against serde_json::Value
│   │
│   ├── bashli-sed/               # sedregex integration — thin wrapper
│   │   ├── Cargo.toml            #   deps: sedregex, serde_json
│   │   └── src/
│   │       └── lib.rs            #   Public API: replace(), replace_all(), apply_commands()
│   │
│   ├── bashli-awk/               # awk-rs integration — thin wrapper
│   │   ├── Cargo.toml            #   deps: awk-rs, serde_json
│   │   └── src/
│   │       └── lib.rs            #   Public API: eval_program(), eval_to_string()
│   │
│   ├── bashli-runner/            # Subprocess execution — nothing else
│   │   ├── Cargo.toml            #   deps: bashli-core, tokio
│   │   └── src/
│   │       ├── lib.rs            #   Re-exports
│   │       ├── command.rs        #   CommandRunner: spawn, capture stdout/stderr
│   │       ├── timeout.rs        #   Timeout enforcement via tokio::time
│   │       └── process_group.rs  #   Process group management for clean kills
│   │
│   ├── bashli-budget/            # Token budget tracking — stateful, self-contained
│   │   ├── Cargo.toml            #   deps: bashli-core
│   │   └── src/
│   │       ├── lib.rs            #   Re-exports
│   │       ├── tracker.rs        #   BudgetTracker: charge(), remaining(), is_exhausted()
│   │       ├── allocator.rs      #   Budget allocation strategies (equal, weighted, etc.)
│   │       └── truncator.rs      #   Smart truncation (head+tail with gap marker)
│   │
│   ├── bashli-steps/             # StepExecutor trait + all built-in step implementations
│   │   ├── Cargo.toml            #   deps: bashli-core, bashli-vars, bashli-runner,
│   │   │                         #         bashli-transforms, bashli-extract, bashli-budget
│   │   └── src/
│   │       ├── lib.rs            #   StepExecutor trait + StepRegistry
│   │       ├── registry.rs       #   StepRegistry: register, dispatch, default_registry()
│   │       ├── cmd.rs            #   CmdStep executor
│   │       │                     #     Uses: runner (spawn), transforms (post-process),
│   │       │                     #     extract (sub-vars), budget (charge output)
│   │       ├── let_step.rs       #   Let executor (pure variable assignment)
│   │       ├── assert.rs         #   Assert executor (condition evaluation)
│   │       ├── if_step.rs        #   If/Else executor (conditional dispatch)
│   │       ├── for_each.rs       #   ForEach executor (iteration + collect modes)
│   │       ├── write.rs          #   Write executor (file I/O, atomic writes)
│   │       ├── read.rs           #   Read executor (file I/O + transform)
│   │       └── context.rs        #   StepContext: shared resources passed to every executor
│   │
│   ├── bashli-engine/            # Orchestrator — thin dispatch loop
│   │   ├── Cargo.toml            #   deps: bashli-core, bashli-vars, bashli-steps,
│   │   │                         #         bashli-budget, bashli-runner, tokio
│   │   └── src/
│   │       ├── lib.rs            #   Engine::run(TaskSpec) -> TaskResult
│   │       ├── sequential.rs     #   Sequential execution mode
│   │       ├── independent.rs    #   Independent execution mode
│   │       ├── parallel.rs       #   Parallel / ParallelN execution modes
│   │       └── builder.rs        #   EngineBuilder: configure registries, settings, runner
│   │
│   └── bashli-cli/               # CLI binary — argument parsing + I/O
│       ├── Cargo.toml            #   deps: bashli-core, bashli-engine, clap, serde_yaml
│       └── src/
│           ├── main.rs           #   Entrypoint: parse args, build engine, run, print result
│           ├── input.rs          #   JSON/YAML parsing, stdin detection, shorthand expansion
│           └── output.rs         #   Output formatting (JSON compact, pretty, debug trace)
│
├── tests/
│   ├── integration/
│   │   ├── basic_exec.rs         #   Simple command execution
│   │   ├── variables.rs          #   Capture, interpolation, scoping
│   │   ├── transforms.rs         #   Every transform variant
│   │   ├── extractions.rs        #   Every extraction method
│   │   ├── control_flow.rs       #   Assert, If/Else branching
│   │   ├── iteration.rs          #   ForEach + collect modes
│   │   ├── file_ops.rs           #   Write/Read steps
│   │   ├── parallel.rs           #   Parallel + ParallelN modes
│   │   ├── budget.rs             #   Token budget enforcement
│   │   ├── extensions.rs         #   Custom StepExecutor + TransformFn registration
│   │   └── e2e_cli.rs            #   Full CLI invocation via assert_cmd
│   └── fixtures/
│       ├── simple_task.json
│       ├── multi_step.json
│       ├── complex_pipeline.json
│       ├── extension_step.json
│       └── expected/             #   Golden output files for snapshot testing
│
└── docs/
    ├── SPEC.md                   #   This document
    ├── EXAMPLES.md               #   Cookbook of common patterns
    ├── AGENT_GUIDE.md            #   Integration guide for AI agents
    └── EXTENDING.md              #   How to add new steps, transforms, extractors
```

### 4.3 Dependency Graph

The dependency graph is a strict **directed acyclic graph** — no cycles, no lateral dependencies between peers. Each crate depends only on crates below it in the hierarchy.

```
                            bashli-cli
                               │
                          bashli-engine
                         ╱      │      ╲
                       ╱        │        ╲
                bashli-steps  bashli-budget  (tokio)
               ╱  │   │  ╲        │
              ╱   │   │   ╲       │
bashli-runner │   │ bashli-transforms
              │   │       │  ╲
              │   │       │ bashli-jq
              │   │       │
              │ bashli-extract
              │       │
              │  bashli-jq (shared)
              │
          bashli-vars
              │
          bashli-core  ← (depended on by ALL crates above)
```

#### Dependency Rules (Enforced)

1. **bashli-core** depends on nothing internal. Only `serde`, `serde_json`.
2. **bashli-vars** depends only on `bashli-core`.
3. **bashli-jq** depends only on `jaq-*` crates and `serde_json` — no bashli internal deps.
4. **bashli-sed** depends only on `sedregex` and `serde_json` — no bashli internal deps.
5. **bashli-awk** depends only on `awk-rs` and `serde_json` — no bashli internal deps.
6. **bashli-transforms** depends on `bashli-core` (for types), `bashli-jq`, `bashli-sed`, and `bashli-awk` (for their respective transforms). Does NOT depend on `bashli-vars` or `bashli-runner`.
7. **bashli-extract** depends on `bashli-core` and `bashli-jq`. Does NOT depend on `bashli-vars` or `bashli-runner`.
8. **bashli-runner** depends on `bashli-core` and `tokio`. Does NOT depend on `bashli-vars`, `bashli-transforms`, or `bashli-steps`.
9. **bashli-budget** depends only on `bashli-core`.
10. **bashli-steps** is the **integration layer** — it depends on everything above because it wires runner + transforms + extract + vars + budget into coherent step execution. But it does NOT depend on `bashli-engine`.
11. **bashli-engine** depends on `bashli-steps` (to dispatch) and `bashli-budget` (to enforce). It does NOT depend on `bashli-transforms` or `bashli-runner` directly — those are accessed indirectly through `bashli-steps`.
12. **bashli-cli** depends on `bashli-engine` and `bashli-core`. It does NOT directly reference any internal crate.

#### Full Dependency Table

| Crate | Internal Deps | External Deps |
|---|---|---|
| `bashli-core` | — | `serde`, `serde_json` |
| `bashli-vars` | `bashli-core` | `regex` |
| `bashli-jq` | — | `jaq-core`, `jaq-parse`, `jaq-std`, `serde_json` |
| `bashli-sed` | — | `sedregex`, `serde_json` |
| `bashli-awk` | — | `awk-rs`, `serde_json` |
| `bashli-transforms` | `bashli-core`, `bashli-jq`, `bashli-sed`, `bashli-awk` | `regex`, `sha2`, `base64` |
| `bashli-extract` | `bashli-core`, `bashli-jq` | `regex` |
| `bashli-runner` | `bashli-core` | `tokio` |
| `bashli-budget` | `bashli-core` | — |
| `bashli-steps` | `bashli-core`, `bashli-vars`, `bashli-runner`, `bashli-transforms`, `bashli-extract`, `bashli-budget` | `async-trait` |
| `bashli-engine` | `bashli-core`, `bashli-vars`, `bashli-steps`, `bashli-budget`, `bashli-runner` | `tokio` |
| `bashli-cli` | `bashli-core`, `bashli-engine` | `clap`, `serde_yaml` |

### 4.4 Compilation Isolation Matrix

This matrix shows what recompiles when you change a crate. The modular design means most changes only trigger partial rebuilds:

| Changed Crate | Also Rebuilds |
|---|---|
| `bashli-core` | Everything (types are foundational) |
| `bashli-vars` | `bashli-steps`, `bashli-engine`, `bashli-cli` |
| `bashli-jq` | `bashli-transforms`, `bashli-extract`, `bashli-steps`, `bashli-engine`, `bashli-cli` |
| `bashli-sed` | `bashli-transforms`, `bashli-steps`, `bashli-engine`, `bashli-cli` |
| `bashli-awk` | `bashli-transforms`, `bashli-steps`, `bashli-engine`, `bashli-cli` |
| `bashli-transforms` | `bashli-steps`, `bashli-engine`, `bashli-cli` |
| `bashli-extract` | `bashli-steps`, `bashli-engine`, `bashli-cli` |
| `bashli-runner` | `bashli-steps`, `bashli-engine`, `bashli-cli` |
| `bashli-budget` | `bashli-steps`, `bashli-engine`, `bashli-cli` |
| `bashli-steps` | `bashli-engine`, `bashli-cli` |
| `bashli-engine` | `bashli-cli` |
| `bashli-cli` | Nothing else |
| **Add new transform** | `bashli-transforms` + downstream only |
| **Add new step type** | `bashli-steps` + downstream only |
| **Add new extractor** | `bashli-extract` + downstream only |

### 4.5 Core Trait: StepExecutor

The central extension point. Every step type implements this trait. The engine never matches on concrete step types — it dispatches through the `StepRegistry`.

```rust
// bashli-steps/src/lib.rs

/// Shared resources available to every step executor.
/// Passed by the engine; executors never construct this themselves.
pub struct StepContext<'a> {
    /// Variable store (read + write access)
    pub vars: &'a mut VarStore,
    /// Command runner (subprocess execution)
    pub runner: &'a CommandRunner,
    /// Token budget tracker
    pub budget: &'a mut BudgetTracker,
    /// Transform registry (for CmdStep post-processing)
    pub transforms: &'a TransformRegistry,
    /// Extraction registry (for CmdStep sub-var extraction)
    pub extractors: &'a ExtractorRegistry,
    /// Step registry (for recursive step execution in ForEach/If)
    pub step_registry: &'a StepRegistry,
    /// Global settings from the TaskSpec
    pub settings: &'a GlobalSettings,
}

/// The trait every step type must implement.
#[async_trait]
pub trait StepExecutor: Send + Sync {
    /// Returns the step kind string for StepResult.kind
    fn kind(&self) -> StepKind;

    /// Validate the step's configuration before execution.
    /// Called during TaskSpec validation (before any step runs).
    fn validate(&self) -> Result<(), ValidationError> {
        Ok(()) // default: no validation
    }

    /// Execute the step, returning a StepResult.
    async fn execute(&self, ctx: &mut StepContext<'_>) -> Result<StepResult, ExecError>;
}
```

#### Step Registration

```rust
// bashli-steps/src/registry.rs

pub struct StepRegistry {
    /// Built-in step executors (keyed by Step enum discriminant)
    builtins: HashMap<&'static str, Box<dyn Fn(&Step) -> Option<Box<dyn StepExecutor>>>>,
    /// Extension step executors (keyed by ExtensionStep.kind)
    extensions: HashMap<String, Box<dyn Fn(&serde_json::Value) -> Result<Box<dyn StepExecutor>, ValidationError>>>,
}

impl StepRegistry {
    /// Create with all built-in step types pre-registered.
    pub fn default_registry() -> Self {
        let mut reg = Self::new();
        reg.register_builtin("cmd", |s| /* ... */);
        reg.register_builtin("let", |s| /* ... */);
        reg.register_builtin("assert", |s| /* ... */);
        reg.register_builtin("if", |s| /* ... */);
        reg.register_builtin("for_each", |s| /* ... */);
        reg.register_builtin("write", |s| /* ... */);
        reg.register_builtin("read", |s| /* ... */);
        reg
    }

    /// Register an extension step type.
    /// Called by plugins to add new step kinds.
    pub fn register_extension<F>(&mut self, kind: &str, factory: F)
    where
        F: Fn(&serde_json::Value) -> Result<Box<dyn StepExecutor>, ValidationError> + 'static,
    {
        self.extensions.insert(kind.to_string(), Box::new(factory));
    }

    /// Dispatch: given a Step, return the appropriate executor.
    pub fn resolve(&self, step: &Step) -> Result<Box<dyn StepExecutor>, ExecError>;
}
```

#### Example: Adding a Custom Step (HttpStep)

A developer extending bashli adds a new file in their own crate (or in `bashli-steps/src/` for first-party):

```rust
// example: bashli-steps-http/src/lib.rs (or bashli-steps/src/http.rs)

use bashli_steps::{StepExecutor, StepContext, StepResult, StepKind};

pub struct HttpStep {
    url: String,
    method: String,
    headers: BTreeMap<String, String>,
    body: Option<String>,
    capture: Option<String>,
}

impl HttpStep {
    pub fn from_config(config: &serde_json::Value) -> Result<Self, ValidationError> {
        // Parse the extension step's config JSON into HttpStep fields
    }
}

#[async_trait]
impl StepExecutor for HttpStep {
    fn kind(&self) -> StepKind {
        StepKind::Extension("http".into())
    }

    fn validate(&self) -> Result<(), ValidationError> {
        if self.url.is_empty() { return Err(ValidationError::new("url is required")); }
        Ok(())
    }

    async fn execute(&self, ctx: &mut StepContext<'_>) -> Result<StepResult, ExecError> {
        let url = ctx.vars.interpolate(&self.url)?;
        // ... make HTTP request, capture response ...
        if let Some(ref var_name) = self.capture {
            ctx.vars.set(var_name, response_value);
        }
        Ok(StepResult { /* ... */ })
    }
}

// Registration (called at startup):
// registry.register_extension("http", |config| Ok(Box::new(HttpStep::from_config(config)?)));
```

Usage in a TaskSpec:

```json
{
  "extension": {
    "kind": "http",
    "config": {
      "url": "https://api.github.com/repos/$OWNER/$REPO/pulls",
      "method": "GET",
      "capture": "$PULLS"
    }
  }
}
```

### 4.6 Core Trait: TransformFn

Each transform implements a pure function trait. No I/O, no async, no side effects — just `&str` in, `Value` out.

```rust
// bashli-transforms/src/lib.rs

/// A named, stateless transform function.
pub trait TransformFn: Send + Sync {
    /// Human-readable name for error messages
    fn name(&self) -> &str;

    /// Apply the transform to the input string.
    /// Returns a serde_json::Value (String, Array, Number, Object, etc.)
    fn apply(&self, input: &str, config: &serde_json::Value) -> Result<Value, TransformError>;
}

/// Registry mapping Transform variants to implementations.
pub struct TransformRegistry {
    builtins: HashMap<&'static str, Box<dyn TransformFn>>,
    extensions: HashMap<String, Box<dyn TransformFn>>,
}

impl TransformRegistry {
    pub fn default_registry() -> Self {
        let mut reg = Self::new();
        reg.register("trim", Box::new(TrimTransform));
        reg.register("lines", Box::new(LinesTransform));
        reg.register("json", Box::new(JsonTransform));
        reg.register("count_lines", Box::new(CountLinesTransform));
        reg.register("count_bytes", Box::new(CountBytesTransform));
        reg.register("count_words", Box::new(CountWordsTransform));
        reg.register("head", Box::new(HeadTransform));
        reg.register("tail", Box::new(TailTransform));
        reg.register("sort", Box::new(SortTransform));
        reg.register("unique", Box::new(UniqueTransform));
        reg.register("jq", Box::new(JqTransform::new()));
        reg.register("sed", Box::new(SedTransform::new()));
        reg.register("awk", Box::new(AwkTransform::new()));
        reg.register("grep", Box::new(GrepTransform));
        reg.register("split", Box::new(SplitTransform));
        reg.register("base64_encode", Box::new(Base64EncodeTransform));
        reg.register("base64_decode", Box::new(Base64DecodeTransform));
        reg.register("sha256", Box::new(Sha256Transform));
        reg.register("code_block", Box::new(CodeBlockTransform));
        reg.register("regex", Box::new(RegexTransform));
        reg
    }

    /// Register a custom transform (plugin extension point)
    pub fn register_extension(&mut self, name: &str, transform: Box<dyn TransformFn>) {
        self.extensions.insert(name.to_string(), transform);
    }

    /// Resolve a Transform enum to a concrete implementation + config.
    /// Handles the Pipe variant by chaining multiple transforms.
    pub fn apply(&self, input: &str, transform: &Transform) -> Result<Value, TransformError>;
}
```

#### Example: Built-in transform implementations (pure functions, one file each)

```rust
// bashli-transforms/src/text.rs

pub struct TrimTransform;
impl TransformFn for TrimTransform {
    fn name(&self) -> &str { "trim" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        Ok(Value::String(input.trim().to_string()))
    }
}

pub struct LinesTransform;
impl TransformFn for LinesTransform {
    fn name(&self) -> &str { "lines" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        let lines: Vec<Value> = input.lines().map(|l| Value::String(l.to_string())).collect();
        Ok(Value::Array(lines))
    }
}

pub struct CountLinesTransform;
impl TransformFn for CountLinesTransform {
    fn name(&self) -> &str { "count_lines" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        Ok(Value::Number(input.lines().count().into()))
    }
}

pub struct CountBytesTransform;
impl TransformFn for CountBytesTransform {
    fn name(&self) -> &str { "count_bytes" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        Ok(Value::Number(input.len().into()))
    }
}

pub struct CountWordsTransform;
impl TransformFn for CountWordsTransform {
    fn name(&self) -> &str { "count_words" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        Ok(Value::Number(input.split_whitespace().count().into()))
    }
}
```

```rust
// bashli-transforms/src/slice.rs

pub struct HeadTransform;
impl TransformFn for HeadTransform {
    fn name(&self) -> &str { "head" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let n = config.as_u64().ok_or(TransformError::InvalidConfig("head requires a number"))? as usize;
        let result: String = input.lines().take(n).collect::<Vec<_>>().join("\n");
        Ok(Value::String(result))
    }
}

pub struct TailTransform;
impl TransformFn for TailTransform {
    fn name(&self) -> &str { "tail" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let n = config.as_u64().ok_or(TransformError::InvalidConfig("tail requires a number"))? as usize;
        let lines: Vec<&str> = input.lines().collect();
        let start = lines.len().saturating_sub(n);
        Ok(Value::String(lines[start..].join("\n")))
    }
}
```

```rust
// bashli-transforms/src/grep.rs

pub struct GrepTransform;
impl TransformFn for GrepTransform {
    fn name(&self) -> &str { "grep" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let spec: GrepSpec = serde_json::from_value(config.clone())
            .map_err(|e| TransformError::InvalidConfig(e))?;

        let re = RegexBuilder::new(&spec.pattern)
            .case_insensitive(spec.ignore_case)
            .build()
            .map_err(TransformError::Regex)?;

        let matches: Vec<&str> = input.lines()
            .filter(|line| re.is_match(line) != spec.invert)
            .collect();

        if spec.count {
            return Ok(Value::Number(matches.len().into()));
        }
        if spec.only_matching {
            let extracts: Vec<Value> = matches.iter()
                .filter_map(|line| re.find(line).map(|m| Value::String(m.as_str().to_string())))
                .collect();
            return Ok(Value::Array(extracts));
        }
        Ok(Value::String(matches.join("\n")))
    }
}
```

### 4.7 Core Trait: ExtractorFn

Same pattern as transforms — pure function, registry-based dispatch:

```rust
// bashli-extract/src/lib.rs

pub trait ExtractorFn: Send + Sync {
    fn name(&self) -> &str;
    fn extract(&self, input: &str, config: &serde_json::Value) -> Result<Value, ExtractionError>;
}

pub struct ExtractorRegistry {
    builtins: HashMap<&'static str, Box<dyn ExtractorFn>>,
    extensions: HashMap<String, Box<dyn ExtractorFn>>,
}

impl ExtractorRegistry {
    pub fn default_registry() -> Self {
        let mut reg = Self::new();
        reg.register("jq", Box::new(JqExtractor::new()));
        reg.register("regex", Box::new(RegexExtractor));
        reg.register("count_matching", Box::new(CountMatchingExtractor));
        reg.register("first_matching", Box::new(FirstMatchingExtractor));
        reg.register("all_matching", Box::new(AllMatchingExtractor));
        reg.register("line", Box::new(LineExtractor));
        reg.register("line_range", Box::new(LineRangeExtractor));
        reg
    }

    pub fn register_extension(&mut self, name: &str, extractor: Box<dyn ExtractorFn>);
    pub fn apply(&self, input: &str, extraction: &Extraction) -> Result<Value, ExtractionError>;
}
```

### 4.8 Engine Architecture

The engine is a thin orchestrator. It owns the execution loop and delegates everything to registries and traits.

```rust
// bashli-engine/src/lib.rs

pub struct Engine {
    step_registry: StepRegistry,
    transform_registry: TransformRegistry,
    extractor_registry: ExtractorRegistry,
    runner: CommandRunner,
    settings: GlobalSettings,
}

impl Engine {
    /// Execute a full TaskSpec, returning the structured result.
    pub async fn run(&self, spec: TaskSpec) -> TaskResult {
        // 1. Validate all steps via registry
        // 2. Initialize VarStore with system vars + let_vars
        // 3. Initialize BudgetTracker from settings
        // 4. Dispatch to mode-specific executor
        // 5. Build and return TaskResult
    }
}
```

```rust
/// Builder pattern for configuring the engine.
/// This is where plugins register their extensions.
pub struct EngineBuilder {
    step_registry: StepRegistry,
    transform_registry: TransformRegistry,
    extractor_registry: ExtractorRegistry,
    runner_config: RunnerConfig,
    settings: GlobalSettings,
}

impl EngineBuilder {
    pub fn new() -> Self {
        Self {
            step_registry: StepRegistry::default_registry(),
            transform_registry: TransformRegistry::default_registry(),
            extractor_registry: ExtractorRegistry::default_registry(),
            runner_config: RunnerConfig::default(),
            settings: GlobalSettings::default(),
        }
    }

    /// Register a custom step type
    pub fn register_step<F>(mut self, kind: &str, factory: F) -> Self
    where F: Fn(&Value) -> Result<Box<dyn StepExecutor>, ValidationError> + 'static
    {
        self.step_registry.register_extension(kind, factory);
        self
    }

    /// Register a custom transform
    pub fn register_transform(mut self, name: &str, transform: Box<dyn TransformFn>) -> Self {
        self.transform_registry.register_extension(name, transform);
        self
    }

    /// Register a custom extractor
    pub fn register_extractor(mut self, name: &str, extractor: Box<dyn ExtractorFn>) -> Self {
        self.extractor_registry.register_extension(name, extractor);
        self
    }

    pub fn build(self) -> Engine { /* ... */ }
}
```

#### Mode-Specific Execution

Each execution mode is a separate module with a single public function:

```rust
// bashli-engine/src/sequential.rs
pub(crate) async fn run_sequential(
    steps: &[Step],
    ctx: &mut StepContext<'_>,
) -> Vec<StepResult> {
    let mut results = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        ctx.vars.set("$_STEP_INDEX", Value::Number(i.into()));
        let executor = ctx.step_registry.resolve(step)?;
        let result = executor.execute(ctx).await?;

        if result.exit_code.unwrap_or(0) != 0 {
            results.push(result);
            break; // Sequential mode: stop on first failure
        }
        results.push(result);
    }
    results
}

// bashli-engine/src/independent.rs
pub(crate) async fn run_independent(
    steps: &[Step],
    ctx: &mut StepContext<'_>,
) -> Vec<StepResult> {
    let mut results = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        ctx.vars.set("$_STEP_INDEX", Value::Number(i.into()));
        let executor = ctx.step_registry.resolve(step)?;
        let result = executor.execute(ctx).await;
        results.push(result.unwrap_or_else(|e| StepResult::from_error(i, e)));
        // Independent mode: always continue
    }
    results
}

// bashli-engine/src/parallel.rs
pub(crate) async fn run_parallel(
    steps: &[Step],
    ctx: &StepContext<'_>,  // Note: shared read-only for parallel
    max_concurrency: Option<usize>,
) -> Vec<StepResult> {
    // Fan out with tokio::spawn, bounded by semaphore
    // Collect with join_all, preserve order
}
```

### 4.9 Variable Store Architecture

Extracted into its own crate because it is runtime logic (interpolation, scoping, path resolution), not just types.

```rust
// bashli-vars/src/store.rs

pub struct VarStore {
    /// Global variables (captures, lets, system vars)
    globals: BTreeMap<String, Value>,
    /// Stack of loop-scoped variables (for nested for_each)
    scopes: Vec<BTreeMap<String, Value>>,
}

impl VarStore {
    pub fn resolve(&self, reference: &str) -> Result<Value, VarError>;
    pub fn interpolate(&self, template: &str) -> Result<String, VarError>;
    pub fn set(&mut self, name: &str, value: Value);
    pub fn push_scope(&mut self);
    pub fn pop_scope(&mut self);
    pub fn set_scoped(&mut self, name: &str, value: Value);
    pub fn keys(&self) -> Vec<&str>;
    pub fn export_summary(&self, keys: &[String]) -> BTreeMap<String, Value>;
}
```

```rust
// bashli-vars/src/interpolate.rs

/// Parse and interpolate a template string containing $VAR references.
///
/// Handles:
///   $VAR           → simple lookup
///   $VAR.field     → JSON path traversal
///   $VAR[0]        → array index
///   ${VAR}_suffix  → explicit boundary
///   $$             → literal $
///
/// All resolved values are shell-escaped before insertion into command strings.
pub fn interpolate(template: &str, store: &VarStore, escape: bool) -> Result<String, VarError>;
```

```rust
// bashli-vars/src/path.rs

/// Resolve a JSON path like "field[2].name" against a serde_json::Value.
pub fn resolve_path(root: &Value, path: &str) -> Result<Value, VarError>;
```

```rust
// bashli-vars/src/escape.rs

/// Shell-escape a string for safe interpolation into a command.
/// Prevents injection: a value like `; rm -rf /` becomes `'; rm -rf /'`.
pub fn shell_escape(value: &str) -> String;
```

### 4.10 Command Runner Architecture

Isolated subprocess management. No business logic, no transforms, no variables.

```rust
// bashli-runner/src/command.rs

pub struct CommandRunner {
    shell: Vec<String>,          // default: ["/bin/sh", "-c"]
    default_timeout: Duration,
}

impl CommandRunner {
    pub fn new(shell: Vec<String>, default_timeout: Duration) -> Self;
    pub async fn run(&self, cmd: &str, opts: &RunOpts) -> Result<RawOutput, ExecError>;
}

pub struct RunOpts {
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub stdout_mode: StdoutMode,
    pub stderr_mode: StderrMode,
    pub stdin_data: Option<Vec<u8>>,
    pub timeout: Duration,
}

pub struct RawOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub duration: Duration,
}
```

```rust
// bashli-runner/src/process_group.rs

/// Spawn a command in its own process group.
/// Returns a handle that can kill the entire group (including children).
pub fn spawn_in_group(cmd: &str, shell: &[String], opts: &RunOpts) -> Result<GroupedChild, ExecError>;

pub struct GroupedChild {
    child: tokio::process::Child,
    pgid: i32,
}

impl GroupedChild {
    pub fn kill_group(&self) -> Result<(), std::io::Error>;
}
```

### 4.11 jaq Integration Architecture

Thin wrapper with compilation caching for repeated filter expressions:

```rust
// bashli-jq/src/compiler.rs

/// Compiled jaq filter cache. Filters are expensive to compile but cheap to evaluate.
pub struct FilterCache {
    cache: HashMap<String, CompiledFilter>,
    max_size: usize,
}

impl FilterCache {
    pub fn compile(&mut self, expr: &str) -> Result<&CompiledFilter, JqError>;
}
```

```rust
// bashli-jq/src/eval.rs

/// Evaluate a jaq filter against a JSON string.
pub fn eval(expr: &str, input: &str) -> Result<Value, JqError>;

/// Evaluate a jaq filter against an already-parsed Value.
pub fn eval_value(expr: &str, input: &Value) -> Result<Value, JqError>;

/// Evaluate and return as string (convenience for transforms).
pub fn eval_to_string(expr: &str, input: &str) -> Result<String, JqError>;
```

### 4.12 sed Integration Architecture (bashli-sed)

Thin wrapper around the `sedregex` crate. Same pattern as bashli-jq: the crate knows nothing about bashli internals — it just exposes sed operations as Rust functions.

```rust
// bashli-sed/src/lib.rs

/// Apply one or more sed substitution commands to input text.
/// Commands use standard sed syntax: "s/pattern/replacement/flags"
///
/// Example: apply("hello world", &["s/world/rust/"])  →  "hello rust"
/// Example: apply("foo FOO", &["s/foo/bar/gi"])       →  "bar BAR"
pub fn apply(input: &str, commands: &[&str]) -> Result<String, SedError>;

/// Apply a single sed command (convenience wrapper).
pub fn replace(input: &str, command: &str) -> Result<String, SedError>;
```

The `sedregex` crate handles sed `s/` syntax natively — pattern, replacement, and flags (`g`, `i`, `m`). This covers the vast majority of sed usage in agent contexts without needing to implement any sed parsing ourselves.

Usage as a bashli transform:

```json
{
  "cmd": "cat config.toml",
  "capture": "$CONFIG",
  "transform": {"sed": "s/localhost/0.0.0.0/g"}
}
```

Multiple commands:

```json
{
  "transform": {"sed": ["s/foo/bar/g", "s/baz/qux/gi"]}
}
```

### 4.13 awk Integration Architecture (bashli-awk)

Thin wrapper around the `awk-rs` crate. Provides a Rust-native awk interpreter — full POSIX awk language with GNU extensions.

```rust
// bashli-awk/src/lib.rs

/// Execute an awk program against input text.
/// Returns the program's stdout as a string.
///
/// Example: eval("{print $2}", "hello world", None)  →  "world\n"
pub fn eval(program: &str, input: &str, opts: &AwkOpts) -> Result<String, AwkError>;

pub struct AwkOpts {
    /// Field separator (-F flag)
    pub field_separator: Option<String>,
    /// Pre-set variables (-v flag)
    pub vars: BTreeMap<String, String>,
}

/// Convenience: extract a single field from each line.
/// Equivalent to: awk -F{sep} '{print ${field}}'
pub fn field(input: &str, field_num: usize, separator: Option<&str>) -> Result<String, AwkError>;
```

The `awk-rs` crate provides `Lexer` → `Parser` → `Interpreter` pipeline that runs entirely in-process. No subprocess, full awk language support including `BEGIN`/`END` blocks, associative arrays, and built-in functions.

Usage as a bashli transform:

```json
{
  "cmd": "ps aux",
  "capture": "$PROCS",
  "transform": {"awk": {"program": "{print $1, $11}", "field_separator": null}}
}
```

Column extraction shorthand:

```json
{
  "cmd": "cat /etc/passwd",
  "capture": "$USERS",
  "transform": {"awk": {"program": "{print $1}", "field_separator": ":"}}
}
```

Full awk programs work too:

```json
{
  "transform": {
    "awk": {
      "program": "BEGIN{sum=0} {sum+=$1} END{print sum}",
      "vars": {"THRESHOLD": "100"}
    }
  }
}

### 4.14 Budget Tracker Architecture

Stateful token-budget enforcement, isolated from execution logic:

```rust
// bashli-budget/src/tracker.rs

pub struct BudgetTracker {
    total: usize,
    consumed: usize,
    allocation: BudgetAllocation,
    overflow: OverflowStrategy,
    step_count: usize,
}

impl BudgetTracker {
    pub fn new(budget: &TokenBudget, step_count: usize) -> Self;
    pub fn charge(&mut self, step_index: usize, output: &str) -> BudgetResult;
    pub fn remaining(&self) -> usize;
    pub fn is_exhausted(&self) -> bool;
    pub fn allocation_for_step(&self, step_index: usize) -> usize;
}

pub enum BudgetResult {
    Accepted(String),
    Truncated { output: String, lines_dropped: usize },
    Dropped,
    Abort,
}
```

```rust
// bashli-budget/src/truncator.rs

/// Smart truncation: keeps head and tail of output with a gap marker.
pub fn smart_truncate(input: &str, max_lines: usize) -> (String, usize);

/// Estimate token count from byte length (rough: ~4 chars per token).
pub fn estimate_tokens(text: &str) -> usize;
```

---

## 5. Engineering Specification

### 5.1 CLI Interface

```
bashli 1.0.0
Structured bash execution engine for AI agents

USAGE:
    bashli [OPTIONS] [SPEC]
    bashli [OPTIONS] -f <FILE>
    cat spec.json | bashli [OPTIONS] -

ARGS:
    <SPEC>    Inline JSON task specification

OPTIONS:
    -f, --file <FILE>        Read task spec from a file
    -y, --yaml               Parse input as YAML instead of JSON
    -v, --verbosity <LEVEL>  Output verbosity [default: normal]
                             [possible values: minimal, normal, full, debug]
    -p, --pretty             Pretty-print JSON output
    -t, --timeout <MS>       Global timeout in milliseconds [default: 300000]
        --dry-run            Validate and show interpolated commands without executing
        --shell <SHELL>      Shell to use [default: /bin/sh -c]
        --no-color           Disable color in debug output
        --read-only          Disable write steps and redirects
        --allowed-paths <GLOB>  Restrict write targets to a glob pattern
        --schema             Print the full JSON Schema for TaskSpec and exit
    -V, --version            Print version
    -h, --help               Print help
```

### 5.2 Shorthand Syntax

For simple invocations, bashli accepts a shorthand JSON format:

```bash
# Minimal: just steps
bashli '{"steps":["ls -la","pwd"]}'

# Steps as a single command (auto-wrapped in array)
bashli '{"cmd":"ls -la"}'

# Ultra-short: bare JSON array is interpreted as steps
bashli '["ls -la","pwd"]'
```

### 5.3 Error Handling Strategy

| Error Type | Behavior |
|---|---|
| Invalid JSON/YAML input | Exit 2, stderr: parse error with line/col |
| Invalid TaskSpec (schema) | Exit 2, stderr: validation errors as JSON |
| Undefined variable | Depends on mode: abort (sequential) or warn (independent) |
| Command timeout | Kill process group, mark step as timeout, follow mode rules |
| Command non-zero exit | Follow mode rules (sequential: abort, independent: continue) |
| File write permission denied | Mark step failed, follow mode rules |
| jaq filter compilation error | Mark step failed with filter error message |
| Token budget exhausted | Truncate or abort depending on overflow strategy |
| Unknown extension step kind | Exit 2, stderr: "unknown step kind: X" |
| Unknown extension transform | Exit 2, stderr: "unknown transform: X" |

All errors are returned in structured JSON, never as raw stderr text. Exit codes: 0 = success, 1 = task failure (a step failed), 2 = spec validation / configuration errors.

### 5.4 Signal Handling

- **SIGINT / SIGTERM**: Kill all running child processes via process group, return partial TaskResult with steps completed so far and `ok: false`.
- **SIGPIPE**: Ignored (output may be piped to head/less by a human, though this is not the primary use case).
- Child processes are spawned in their own process group via `bashli-runner/src/process_group.rs` so timeouts can kill the entire tree.

### 5.5 Security Considerations

bashli executes arbitrary shell commands by design — it is a power tool for trusted agents. However:

1. **No shell injection from variables.** Variable interpolation into `cmd` strings uses proper escaping via `bashli-vars/src/escape.rs`. A captured variable containing `; rm -rf /` is single-quoted before interpolation.
2. **File write guardrails.** The `write` step respects `create_new` mode to prevent accidental overwrites. The `--allowed-paths` flag restricts write targets to a glob pattern. The `--read-only` flag disables all Write steps entirely.
3. **Timeout enforcement.** Every command has a timeout via `bashli-runner/src/timeout.rs`. Runaway processes cannot block indefinitely.
4. **No network by default.** bashli itself makes no network calls. Network behavior comes from the commands the user specifies (or extension steps).
5. **Extension sandboxing.** Extension steps execute within the same `StepContext` and are subject to the same budget, timeout, and path restrictions as built-in steps.

### 5.6 Performance Targets

| Metric | Target |
|---|---|
| Startup overhead (parse + validate) | < 5ms |
| Per-step overhead (spawn + capture) | < 3ms |
| jaq filter compilation (typical expression) | < 1ms |
| jaq filter cache hit | < 0.01ms |
| Parallel mode fan-out (10 commands) | < 10ms overhead |
| Variable interpolation (100-variable store) | < 0.1ms |
| Transform registry dispatch | < 0.01ms |
| Step registry dispatch | < 0.01ms |
| Max concurrent parallel steps | 64 (configurable) |

### 5.7 Testing Strategy

Each crate is independently testable. No integration test requires more than its declared dependencies.

| Crate | Test Type | What's Tested |
|---|---|---|
| `bashli-core` | Unit | Serde round-trips for all types, validation rules, error Display impls |
| `bashli-vars` | Unit | Interpolation (simple, nested, escape, boundary), path resolution, scope push/pop, shell escaping |
| `bashli-jq` | Unit | Filter compilation, evaluation, cache hits/eviction, error messages |
| `bashli-transforms` | Unit (per file) | Each transform variant with known input/output pairs. No I/O, no async — pure functions. |
| `bashli-extract` | Unit (per file) | Each extraction method with known input/output. Pure functions. |
| `bashli-runner` | Integration | Subprocess spawn, timeout kill, stderr merge, exit codes, process group cleanup. Requires real shell. |
| `bashli-budget` | Unit | Budget allocation math, charge/truncate/exhaustion, smart truncation output |
| `bashli-steps` | Integration | Each step executor with a real `StepContext`. Tests variable side effects, runner interaction, transform application. |
| `bashli-engine` | Integration | Full TaskSpec execution for each mode. Multi-step pipelines with assertions on final TaskResult. |
| `bashli-cli` | E2E | Invoke the compiled binary via `assert_cmd`. Check stdout JSON, exit codes, error formatting. |
| Cross-crate | E2E | Fixture-based: JSON task files in `tests/fixtures/`, golden outputs in `tests/fixtures/expected/`. Snapshot testing. |
| Extensions | Integration | Register a custom StepExecutor + TransformFn, execute a TaskSpec using them, verify dispatch. |

### 5.8 Benchmarks

Performance-critical paths get dedicated benchmarks using `criterion`:

| Benchmark | Crate | What's Measured |
|---|---|---|
| `bench_interpolate` | `bashli-vars` | Interpolation throughput at 10, 50, 100 variables |
| `bench_path_resolve` | `bashli-vars` | Deep JSON path resolution (5-level nesting) |
| `bench_transforms` | `bashli-transforms` | Each transform on 1KB, 100KB, 1MB inputs |
| `bench_jq_compile` | `bashli-jq` | Filter compilation cold + cached |
| `bench_grep` | `bashli-transforms` | Regex grep on 10K-line inputs |
| `bench_budget` | `bashli-budget` | Budget allocation + charge for 100 steps |
| `bench_e2e` | `bashli-engine` | Full 10-step TaskSpec execution |

---

## 6. Agent Integration Guide

### 6.1 Claude Code Integration

An agent that currently calls:

```
Bash(grep -r "pub fn defs\|DEFS" ~/.cargo/registry/src/*/jaq-core-1.5.1/src/ 2>/dev/null | head -10 && echo "---" && grep "pub fn parse" ~/.cargo/registry/src/*/jaq-parse-1.0.3/src/lib.rs 2>/dev/null)
```

Instead calls:

```
Bash(bashli '{"description":"Find jaq API surface","mode":"independent","steps":[{"cmd":"grep -r \"pub fn defs\\|DEFS\" ~/.cargo/registry/src/*/jaq-core-1.5.1/src/","limit":{"max_lines":10,"strategy":"head"}},{"cmd":"grep \"pub fn parse\" ~/.cargo/registry/src/*/jaq-parse-1.0.3/src/lib.rs"}]}')
```

The benefits:

- **No newlines** — single string, no multi-command prompt
- **No `#` comments** — `description` field replaces them
- **No `| head -10`** — `limit` handles it natively
- **No `&&` chains** — `mode: independent` runs all steps
- **No `2>/dev/null`** — `stderr: "discard"` replaces it; `stderr: "merge"` is the default so `2>&1` is never needed
- **No `>` or `>>`** — `stdout` routing and `write` steps replace all file redirects
- **Structured output** — agent gets JSON with per-step exit codes

### 6.2 CLAUDE.md Rules

```markdown
## Bash Execution
- Always use `bashli` for multi-step bash operations
- Use `bashli` for any command that would require pipes to `head`, `tail`, `grep`, `jq`, `wc`, or `sort`
- Use inline `Bash()` only for trivial single commands with no pipes
- Always specify `summary` to minimize output token usage
- Use `description` field instead of bash comments
- Use `limit` instead of piping to `head`/`tail`
- Use `transform.grep` instead of piping to `grep`
- Use `transform.jq` instead of piping to `jq`
- Use `write` steps instead of output redirection
```

### 6.3 MCP Server Mode (Future)

bashli can also run as an MCP tool server, exposing `execute_task` as a tool:

```json
{
  "name": "execute_task",
  "description": "Execute a structured bash task with variable capture and transforms",
  "input_schema": { "$ref": "TaskSpec" }
}
```

This allows agents to call bashli natively without going through the Bash tool at all — eliminating the entire class of safety-prompt issues.

Launch: `bashli --mcp` starts a stdio MCP server.

---

## 7. Extensibility Guide

### 7.1 Adding a New Step Type

1. Create a struct implementing `StepExecutor` (see §4.5 for the trait and example).
2. Implement `kind()`, `validate()`, and `execute()`.
3. Register it with `EngineBuilder::register_step()`.
4. Users invoke it via `"extension": {"kind": "your_step", "config": {...}}`.

No existing code is modified. The engine dispatches through the registry.

**First-party steps** (shipped with bashli) are added as new files in `bashli-steps/src/` and registered in `StepRegistry::default_registry()`.

**Third-party steps** are added in separate crates that depend on `bashli-steps` and register via `EngineBuilder`.

### 7.2 Adding a New Transform

1. Create a struct implementing `TransformFn` (see §4.6).
2. Implement `name()` and `apply()`. Transforms are pure functions — no I/O, no async.
3. Register with `EngineBuilder::register_transform()`.
4. Users invoke via `"transform": {"extension": {"name": "your_transform", "config": {...}}}`.

**First-party transforms** are added as new files in `bashli-transforms/src/` and registered in `TransformRegistry::default_registry()`.

### 7.3 Adding a New Extractor

Same pattern as transforms:

1. Implement `ExtractorFn`.
2. Register with `EngineBuilder::register_extractor()`.
3. Invoke via `"extract": {"$VAR": {"extension": {"name": "your_extractor", "config": {...}}}}`.

### 7.4 Extension Crate Template

```toml
# Cargo.toml for a bashli extension
[package]
name = "bashli-ext-http"
version = "0.1.0"

[dependencies]
bashli-core = { path = "../bashli/crates/bashli-core" }
bashli-steps = { path = "../bashli/crates/bashli-steps" }
async-trait = "0.1"
reqwest = { version = "0.12", features = ["json"] }
serde_json = "1"
```

```rust
// src/lib.rs
use bashli_steps::{StepExecutor, StepContext, StepResult};

pub struct HttpStep { /* ... */ }

#[async_trait]
impl StepExecutor for HttpStep { /* ... */ }

/// Register this extension with an EngineBuilder.
pub fn register(builder: bashli_engine::EngineBuilder) -> bashli_engine::EngineBuilder {
    builder.register_step("http", |config| {
        Ok(Box::new(HttpStep::from_config(config)?))
    })
}
```

---

## 8. Cookbook: Common Patterns

### 8.1 Codebase Exploration

```json
{
  "description": "Map the Rust project structure",
  "mode": "independent",
  "steps": [
    {"cmd": "find src -name '*.rs' -type f", "capture": "$FILES", "transform": "lines"},
    {
      "for_each": "$FILES", "as": "$F",
      "steps": [{"cmd": "wc -l < $F", "capture": "$LC", "transform": "trim"}],
      "collect": "map", "capture": "$LINE_COUNTS"
    },
    {"cmd": "cargo metadata --format-version 1", "capture": "$META", "transform": "json"}
  ],
  "summary": ["$FILES", "$LINE_COUNTS", "$META.workspace_root"]
}
```

### 8.2 Build-Check-Fix Cycle

```json
{
  "description": "Check compilation and extract errors",
  "mode": "sequential",
  "steps": [
    {
      "cmd": "cargo check", "capture": "$CHECK",
      "extract": {
        "$ERRORS": {"count_matching": "^error"},
        "$WARNINGS": {"count_matching": "^warning"}
      }
    },
    {
      "if": {"var": "$ERRORS", "greater_than": 0},
      "then": [
        {
          "cmd": "cargo check", "capture": "$DETAIL",
          "transform": {"grep": {"pattern": "^error", "ignore_case": false}},
          "limit": {"max_lines": 10, "strategy": "head"}
        }
      ]
    }
  ],
  "summary": ["$ERRORS", "$WARNINGS", "$DETAIL"]
}
```

### 8.3 Search and Replace Across Files

```json
{
  "description": "Rename function across codebase",
  "steps": [
    {
      "cmd": "grep -rl 'old_function_name' src/",
      "capture": "$AFFECTED", "transform": "lines"
    },
    {
      "assert": "$AFFECTED", "is_not_empty": true,
      "message": "No files contain old_function_name",
      "on_fail": "abort"
    },
    {
      "for_each": "$AFFECTED", "as": "$FILE",
      "steps": [
        {"read": {"path": "$FILE", "capture": "$CONTENT"}},
        {
          "let": {
            "$NEW_CONTENT": {"replace": {"in": "$CONTENT", "find": "old_function_name", "with": "new_function_name"}}
          }
        },
        {"write": {"path": "$FILE", "content": "$NEW_CONTENT", "mode": "create"}}
      ],
      "collect": "array", "capture": "$RESULTS"
    },
    {"cmd": "cargo check", "capture": "$VERIFY", "extract": {"$ERRORS": {"count_matching": "^error"}}}
  ],
  "summary": ["$AFFECTED", "$ERRORS"]
}
```

### 8.4 Parallel Crate Analysis

```json
{
  "description": "Analyze all workspace crates in parallel",
  "mode": "sequential",
  "steps": [
    {
      "cmd": "cargo metadata --format-version 1",
      "capture": "$META", "transform": "json"
    },
    {
      "let": {"$CRATES": "$META.workspace_members"}
    },
    {
      "for_each": "$CRATES", "as": "$CRATE",
      "concurrency": 4,
      "steps": [
        {"cmd": "cargo check -p $CRATE", "capture": "$RESULT", "transform": {"tail": 3}}
      ],
      "collect": "map", "capture": "$CRATE_STATUS"
    }
  ],
  "summary": ["$CRATE_STATUS"]
}
```

---

## 9. Roadmap

Each version targets a specific theme and closes specific bash parity gaps identified in the gap analysis (Appendix E). The "Closes gap" annotations reference that table.

### v1.0 — Core Engine

**Theme:** Minimum viable agent tool. Covers the 80% case: multi-step command execution with structured output.

**Core features:**
- TaskSpec parsing (JSON)
- Sequential and Independent execution modes
- Variable capture and interpolation (`bashli-vars`)
- All built-in Transform variants (`bashli-transforms` + `bashli-jq`)
- All built-in Extraction methods (`bashli-extract`)
- LimitSpec + smart truncation
- Write/Read file operations
- Assert steps
- Token budget management (`bashli-budget`)
- CLI with all flags
- `StepRegistry`, `TransformRegistry`, `ExtractorRegistry` with extension points
- `EngineBuilder` API

**Gap closers in v1.0:**
- **`Sed` transform** — `{"sed": "s/old/new/g"}`. Delegates to the `sedregex` crate for full sed `s/` command syntax with regex support, global/case-insensitive flags. Replaces `| sed`. Registered as a built-in in `TransformRegistry`. *(Closes gap: sed)*
- **`Awk` transform** — `{"awk": {"program": "{print $2}", "field_separator": ":"}}`. Delegates to the `awk-rs` crate for full POSIX awk + GNU extensions. Replaces `| awk`, `| cut -d -f`. Registered as a built-in in `TransformRegistry`. *(Closes gap: awk, cut)*
- **`stderr` / `stdout` I/O routing** — Dedicated JSON fields on every `CmdStep` and `GlobalSettings` that replace ALL redirect operators. `stderr: "merge"` (default, replaces `2>&1`), `stderr: "discard"` (replaces `2>/dev/null`), `stdout: {"file": {...}}` (replaces `>`/`>>`), `stdin` field (replaces `echo $X | cmd` and `cmd < file`). Commands containing redirect operators are rejected at validation time. *(Closes gap: all redirect patterns)*

### v1.1 — Control Flow + Bash Parity

**Theme:** Close the remaining control flow and variable manipulation gaps to reach near-complete bash script parity.

**Core features:**
- If/Else branching
- ForEach iteration with all collect modes
- Parallel and ParallelN execution modes
- Retry logic with exponential backoff
- Nested step execution (ForEach/If containing sub-steps)

**Gap closers in v1.1:**

- **Default values in interpolation** — `${VAR:-fallback}` syntax. If `$VAR` is undefined or empty, substitute `fallback`. Also supports `${VAR:?error message}` to abort with a message if undefined. Implemented in `bashli-vars/src/interpolate.rs`. *(Closes gap: `${VAR:-default}`)*

- **Inline string manipulation** — `${VAR//old/new}` (replace all), `${VAR/old/new}` (replace first), `${VAR%%suffix}` (strip suffix), `${VAR##prefix}` (strip prefix), `${VAR,,}` (lowercase), `${VAR^^}` (uppercase). Implemented as extensions to the interpolation parser in `bashli-vars`. These are resolved at interpolation time, before the value is passed to a command or transform. *(Closes gap: `${VAR//old/new}` and parameter expansion)*

- **`While` step** — Condition-based repetition:
  ```json
  {
    "while": {"var": "$STATUS", "not_equals": "ready"},
    "max_iterations": 20,
    "steps": [
      {"cmd": "curl -s http://localhost:8080/health", "capture": "$STATUS", "transform": "trim"}
    ],
    "delay_ms": 1000
  }
  ```
  Implements `StepExecutor` in `bashli-steps/src/while_step.rs`. The `max_iterations` guard is mandatory to prevent infinite loops in agent contexts. *(Closes gap: while/until loops)*

- **Filesystem assertion conditions** — New variants added to `AssertCondition`:
  ```rust
  enum AssertCondition {
      // ... existing variants ...
      /// File exists at path (replaces `test -f`)
      FileExists(String),
      /// Directory exists at path (replaces `test -d`)
      DirExists(String),
      /// File contains a substring (replaces `grep -q`)
      FileContains { path: String, pattern: String },
      /// File is newer than another file (replaces `test file1 -nt file2`)
      FileNewer { path: String, than: String },
  }
  ```
  These resolve paths through variable interpolation before checking. Implemented in `bashli-steps/src/assert.rs`. *(Closes gap: `test -f`, `test -d`, filesystem predicates)*

- **Math expressions in `let` bindings** — `let` values starting with `=` are evaluated as arithmetic:
  ```json
  {
    "let": {
      "$TOTAL": "= $COUNT_A + $COUNT_B",
      "$AVERAGE": "= $TOTAL / $NUM_FILES",
      "$IS_OVER_LIMIT": "= $TOTAL > 1000"
    }
  }
  ```
  Supports `+`, `-`, `*`, `/`, `%`, `>`, `<`, `>=`, `<=`, `==`, `!=`, `&&`, `||`, and parenthesized grouping. Implemented as a simple expression evaluator in `bashli-vars/src/math.rs` operating on `f64` values. Boolean results are stored as `true`/`false` strings. *(Closes gap: `$(( ))` arithmetic)*

- **`Case` step** — Multi-branch dispatch (replaces chained if/else):
  ```json
  {
    "case": "$FILE_EXT",
    "branches": {
      "rs": [{"cmd": "cargo check", "capture": "$RESULT"}],
      "py": [{"cmd": "python3 -m py_compile $FILE", "capture": "$RESULT"}],
      "ts": [{"cmd": "npx tsc --noEmit $FILE", "capture": "$RESULT"}]
    },
    "default": [{"let": {"$RESULT": "unknown file type"}}]
  }
  ```
  Branch keys support exact match, glob patterns (`*.rs`), and regex (`/pattern/`). Implemented as `bashli-steps/src/case_step.rs`. *(Closes gap: case/switch)*

### v1.2 — Agent Ergonomics + Robustness

**Theme:** Make bashli production-ready for continuous agent use. Add resilience patterns and developer experience features.

**Core features:**
- YAML input support
- Shorthand syntax variants
- `--dry-run` mode
- `bashli --schema` for JSON Schema generation
- Streaming output mode (JSONL per step as they complete)

**Gap closers in v1.2:**

- **`Finally` block** — Steps that always execute, regardless of task success or failure:
  ```json
  {
    "steps": [
      {"cmd": "docker run -d --name test-db postgres", "capture": "$CONTAINER_ID"},
      {"cmd": "cargo test --lib", "capture": "$RESULTS"}
    ],
    "finally": [
      {"cmd": "docker rm -f $CONTAINER_ID"}
    ]
  }
  ```
  The `finally` array is a top-level TaskSpec field. Steps in `finally` always run even if the main steps abort. Their results are included in the TaskResult under a `finally_steps` array. If a finally step fails, it is logged as a warning but does not override the main task's ok/error status. *(Closes gap: `trap cleanup EXIT`)*

- **`Tee` mode on CmdStep** — Simultaneously capture output to a variable AND write to a file:
  ```json
  {
    "cmd": "cargo build --release",
    "capture": "$BUILD_LOG",
    "tee": {"path": "/tmp/build.log", "mode": "create"}
  }
  ```
  The `tee` field accepts the same schema as a `WriteStep`. Output is written to the file AND stored in the captured variable. Implemented in `bashli-steps/src/cmd.rs` as a post-execution hook. *(Closes gap: `| tee file`)*

- **`Pipe` step** — Streaming pipe between two or more processes:
  ```json
  {
    "pipe": [
      {"cmd": "cat /var/log/syslog"},
      {"cmd": "grep 'error'"},
      {"cmd": "sort -u"}
    ],
    "capture": "$RESULT"
  }
  ```
  Connects stdout→stdin between processes using OS pipes (`tokio::process::Command` with `Stdio::piped()`). Unlike transform chains (which buffer between steps), this is true streaming — the processes run concurrently with back-pressure. Only the final process's stdout is captured. Implemented as `bashli-steps/src/pipe_step.rs`. *(Closes gap: streaming `cmd1 | cmd2 | cmd3`)*

  Note: for most agent use cases, transform chains are preferred because they produce structured intermediate results. The `Pipe` step is the escape hatch for performance-sensitive streaming between heavyweight commands.

- **Diff-aware file writes** — `WriteStep` gains a `diff: true` option. When enabled, bashli snapshots the file before writing and includes a unified diff in the StepResult's `note` field. Useful for agents that need to verify their own mutations.

- **`Xargs` mode on ForEach** — Batch iteration that passes multiple items per command invocation:
  ```json
  {
    "for_each": "$FILES",
    "as": "$BATCH",
    "batch_size": 10,
    "steps": [{"cmd": "grep -l 'TODO' $BATCH"}],
    "collect": "concat",
    "capture": "$MATCHES"
  }
  ```
  When `batch_size` is set, the loop variable receives space-separated batches instead of individual items. This mirrors `xargs -n 10` behavior — fewer process spawns, better performance for commands that accept multiple arguments. *(Closes gap: `| xargs cmd`)*

### v1.3 — Reusability + Composition

**Theme:** Enable TaskSpec reuse, composition, and sub-routine patterns. Close the last scripting gaps.

**Core features:**
- MCP server mode (`--mcp`)
- Template library (reusable TaskSpec fragments via `$import`)
- `bashli-ext-*` crate ecosystem

**Gap closers in v1.3:**

- **`$import` directive** — Include reusable TaskSpec fragments from files:
  ```json
  {
    "steps": [
      {"$import": "./fragments/setup-db.json"},
      {"cmd": "cargo test", "capture": "$RESULTS"},
      {"$import": "./fragments/teardown-db.json"}
    ]
  }
  ```
  Fragments are TaskSpec files whose `steps` array is spliced into the parent at the import point. Variables from the parent are available to the fragment and vice versa. Fragments can themselves contain `$import` (with cycle detection). Resolved at parse time in `bashli-cli/src/input.rs`. *(Closes gap: `source script.sh`)*

- **`Function` definitions** — Named, reusable step sequences within a TaskSpec:
  ```json
  {
    "functions": {
      "check_crate": {
        "params": ["$CRATE_NAME"],
        "steps": [
          {"cmd": "cargo check -p $CRATE_NAME", "capture": "$CHECK_OUT"},
          {"cmd": "cargo test -p $CRATE_NAME", "capture": "$TEST_OUT", "transform": {"tail": 5}}
        ],
        "returns": "$TEST_OUT"
      }
    },
    "steps": [
      {"call": "check_crate", "args": {"$CRATE_NAME": "bashli-core"}, "capture": "$CORE_RESULT"},
      {"call": "check_crate", "args": {"$CRATE_NAME": "bashli-vars"}, "capture": "$VARS_RESULT"}
    ]
  }
  ```
  Functions are defined in a top-level `functions` map. A `Call` step type invokes them with argument bindings. Parameters create a new variable scope (like `for_each`). The `returns` field names which captured variable becomes the call's output. Implemented as `bashli-steps/src/call_step.rs`. *(Closes gap: `function name() {}`)*

- **First-party extension steps** — Shipped as optional crates:
  - `bashli-ext-http` — `HttpStep` for REST API calls with JSON response parsing
  - `bashli-ext-sql` — `SqlStep` for database queries (SQLite, PostgreSQL via connection string)
  - `bashli-ext-git` — `GitStep` for structured git operations (status, diff, log as JSON)

### v1.4 — Performance + Advanced Runtime

**Theme:** Optimize for large-scale agent workloads and advanced execution patterns.

**Features:**
- Caching layer (hash command string + captured variable inputs → skip if cached, return stored result)
- Watch mode (re-execute on file changes, via `notify` crate)
- WASM plugin system (custom transforms/steps loadable at runtime without recompilation)
- Metrics export (Prometheus format for build dashboards)
- **Connection pooling for Pipe steps** — Reuse OS pipes across iterations in `for_each` with `pipe` sub-steps
- **Incremental execution** — Given a previous TaskResult, re-run only steps whose inputs (interpolated variables) have changed
- **DAG execution mode** — Steps declare dependencies on captured variables; engine builds a dependency graph and executes with maximum parallelism while respecting data flow:
  ```json
  {
    "mode": "dag",
    "steps": [
      {"cmd": "cmd_a", "capture": "$A"},
      {"cmd": "cmd_b", "capture": "$B"},
      {"cmd": "cmd_c $A $B", "capture": "$C", "depends": ["$A", "$B"]},
      {"cmd": "cmd_d $A", "capture": "$D", "depends": ["$A"]}
    ]
  }
  ```
  Steps `a` and `b` run in parallel. Step `d` starts as soon as `$A` is ready. Step `c` waits for both `$A` and `$B`. Maximum concurrency with correct ordering.

---

### Version Summary: Bash Parity Closure Timeline

| Gap | Bash Equivalent | bashli Version |
|---|---|---|
| sed | `sed 's/old/new/g'` | v1.0 (`Sed` transform via `sedregex`) |
| awk | `awk '{print $2}'` | v1.0 (`Awk` transform via `awk-rs`) |
| cut | `cut -d',' -f2` | v1.0 (`Awk` transform covers this) |
| Stderr discard | `2>/dev/null` | v1.0 (`stderr: "discard"`) |
| Stdin piping | `echo $X \| cmd`, `cmd < file` | v1.0 (`stdin` field + I/O routing) |
| Default values | `${VAR:-default}` | v1.1 |
| String manipulation | `${VAR//old/new}`, `${VAR^^}` | v1.1 |
| While loops | `while [ cond ]; do ... done` | v1.1 (`While` step) |
| Filesystem tests | `test -f`, `test -d` | v1.1 (assert conditions) |
| Arithmetic | `$(( ))` | v1.1 (math in `let`) |
| Case/switch | `case $X in ...` | v1.1 (`Case` step) |
| Cleanup on exit | `trap cleanup EXIT` | v1.2 (`finally` block) |
| Tee to file | `\| tee file` | v1.2 (`tee` on CmdStep) |
| Streaming pipes | `cmd1 \| cmd2 \| cmd3` | v1.2 (`Pipe` step) |
| Xargs batching | `\| xargs -n 10 cmd` | v1.2 (`batch_size` on ForEach) |
| Source/include | `source script.sh` | v1.3 (`$import`) |
| Functions | `function name() {}` | v1.3 (`Function` definitions) |
| DAG parallelism | Complex `&`/`wait` patterns | v1.4 (`dag` mode) |

---

## Appendix A: Full JSON Schema

The complete JSON Schema for TaskSpec validation is published alongside the binary and available via `bashli --schema`.

## Appendix B: Comparison with Alternatives

| Feature | Raw Bash | bashli | Makefile | just | Deno scripts |
|---|---|---|---|---|---|
| Structured I/O | No | JSON | No | No | Possible |
| Variable capture | Manual | Built-in | Manual | Manual | Manual |
| Token budget | No | Built-in | No | No | No |
| Agent-friendly | No | Designed for | No | No | Partial |
| jq built-in | Pipe to jq | jaq native | No | No | No |
| Parallel execution | `&` + `wait` | Declarative | `-j` flag | No | Possible |
| Timeout per command | `timeout` cmd | Built-in | No | No | Possible |
| Single binary | N/A | Yes | Yes | Yes | No |
| Retry logic | Manual loops | Declarative | No | No | Manual |
| Assertions | `test` + `&&` | Declarative | No | No | Manual |
| Extensible step types | No | Trait + Registry | No | No | Possible |
| Plugin ecosystem | No | Cargo crates | No | No | npm |

## Appendix C: Estimated Implementation Effort

### v1.0 Scope

| Crate | Estimated Lines | Complexity | Notes |
|---|---|---|---|
| `bashli-core` | ~650 | Low | Types + serde derives. No logic. Includes `Sed`/`Awk` transform types, `StderrMode`/`StdoutMode`/`stdin` fields. |
| `bashli-vars` | ~500 | Medium | Interpolation parser, JSON path resolution, escaping |
| `bashli-jq` | ~200 | Low | Thin jaq wrapper + LRU cache |
| `bashli-sed` | ~50 | Low | Thin sedregex wrapper — single file |
| `bashli-awk` | ~80 | Low | Thin awk-rs wrapper — single file |
| `bashli-transforms` | ~1,000 | Low | ~15 small pure-function files (~60 lines each), including sed + awk delegates |
| `bashli-extract` | ~300 | Low | ~6 small pure-function files |
| `bashli-runner` | ~300 | Medium | Async subprocess, process groups, timeout, stdin piping |
| `bashli-budget` | ~200 | Low | Math + truncation |
| `bashli-steps` | ~950 | Medium | 7 step executors + registry + context |
| `bashli-engine` | ~400 | Medium | 3 execution modes + builder |
| `bashli-cli` | ~250 | Low | Clap + I/O |
| Tests | ~2,000 | Medium | Unit per crate + integration + e2e + extension tests |
| **v1.0 Total** | **~6,880** | | |

### v1.1 Incremental Additions

| Addition | Estimated Lines | Crate(s) Affected |
|---|---|---|
| Default values + string manipulation in interpolation | ~200 | `bashli-vars` (+interpolate.rs) |
| Math expression evaluator | ~250 | `bashli-vars` (+math.rs) |
| `While` step executor | ~120 | `bashli-steps` (+while_step.rs) |
| `Case` step executor | ~150 | `bashli-steps` (+case_step.rs) |
| Filesystem assert conditions | ~80 | `bashli-steps` (assert.rs) |
| Tests for v1.1 features | ~500 | tests/ |
| **v1.1 Total** | **~1,300** | |

### v1.2 Incremental Additions

| Addition | Estimated Lines | Crate(s) Affected |
|---|---|---|
| `finally` block support | ~100 | `bashli-core` (spec.rs), `bashli-engine` |
| `tee` on CmdStep | ~60 | `bashli-steps` (cmd.rs) |
| `Pipe` step executor (streaming) | ~200 | `bashli-steps` (+pipe_step.rs), `bashli-runner` |
| `batch_size` on ForEach | ~80 | `bashli-steps` (for_each.rs) |
| Diff-aware file writes | ~100 | `bashli-steps` (write.rs) |
| YAML input support | ~50 | `bashli-cli` (input.rs) |
| JSONL streaming output | ~80 | `bashli-cli` (output.rs) |
| `--dry-run` mode | ~60 | `bashli-engine` |
| Tests for v1.2 features | ~500 | tests/ |
| **v1.2 Total** | **~1,230** | |

### v1.3 Incremental Additions

| Addition | Estimated Lines | Crate(s) Affected |
|---|---|---|
| `$import` directive + cycle detection | ~200 | `bashli-cli` (input.rs) |
| `Function` definitions + `Call` step | ~250 | `bashli-core`, `bashli-steps` (+call_step.rs) |
| MCP server mode | ~300 | `bashli-cli` or new `bashli-mcp` crate |
| `bashli-ext-http` | ~400 | New crate |
| `bashli-ext-sql` | ~500 | New crate |
| `bashli-ext-git` | ~350 | New crate |
| Tests for v1.3 features | ~600 | tests/ |
| **v1.3 Total** | **~2,600** | |

### Cumulative Totals

| Version | New Lines | Cumulative Lines |
|---|---|---|
| v1.0 | ~6,880 | ~6,880 |
| v1.1 | ~1,300 | ~8,180 |
| v1.2 | ~1,230 | ~9,410 |
| v1.3 | ~2,600 | ~12,010 |

## Appendix D: Why 12 Crates?

The 12-crate design may seem like over-decomposition. Here is the rationale:

| Concern | Monolith Pain | Modular Benefit |
|---|---|---|
| Adding a transform | Rebuild the entire execution engine | Rebuild only `bashli-transforms` (~800 lines) |
| Adding a step type | Modify the engine's match arms | Implement a trait in a new file, register it |
| Testing transforms | Must mock the runner, vars, budget | Pure functions — no mocks needed, no async |
| Testing variables | Must bring in execution logic | `bashli-vars` has zero I/O, testable in isolation |
| Compilation time | One change rebuilds everything | Incremental: only changed crate + dependents |
| Onboarding | New contributor must understand the whole system | Contributor reads one 200-line crate |
| Third-party extensions | Fork the whole project | Depend on `bashli-steps` (trait) + `bashli-core` (types) |
| Feature flags | Complex conditional compilation | Optional crates: don't compile `bashli-jq` if you don't need jq |

## Appendix E: Bash Parity Gap Analysis

Comprehensive mapping of bash scripting features to bashli equivalents, coverage status, and version timeline.

### Fully Covered (v1.0)

| Bash Feature | bashli Equivalent | Notes |
|---|---|---|
| `cmd1 && cmd2 && cmd3` | `mode: "sequential"` | Better — structured error per step |
| `cmd1; cmd2; cmd3` | `mode: "independent"` | Better — always get all results |
| `# comments` | `description` field | Better — doesn't trigger agent parser |
| `> file`, `>> file` | `stdout: {"file": {...}}` or `write` step | Better — JSON field, no redirect syntax |
| `2> file`, `2>> file` | `stderr: {"file": {...}}` | Better — JSON field |
| `> /dev/null` | `stdout: "discard"` | Better — JSON field |
| `2>/dev/null` | `stderr: "discard"` | Better — JSON field |
| `> /dev/null 2>&1` | `stdout: "discard", stderr: "discard"` | Better — two JSON fields |
| `&> file` | `stdout: {"file": {...}}, stderr: "merge"` | Better — explicit routing |
| `2>&1` | `stderr: "merge"` (the default) | Better — no syntax needed at all |
| `\| tee file` | `stdout: {"tee": {...}}` | Better — JSON field |
| `echo $X \| cmd` | `stdin` field on CmdStep | Better — no pipe needed |
| `cmd < file` | `stdin` field + `read` step | Better — two structured steps |
| `cmd <<< "text"` | `stdin: "literal text"` | Better — JSON field |
| `cat file` | `read` step | Better — transform on read |
| `\| head -N`, `\| tail -N` | `limit` / `transform` | Better — smart truncation |
| `\| grep`, `\| grep -v -i -c -o` | `transform.grep` | Full parity |
| `\| jq 'expr'` | `transform.jq` (jaq) | ~95% parity (some exotic builtins missing) |
| `\| sed 'expr'` | `transform.sed` (sedregex) | Full `s/` command parity |
| `\| awk 'program'` | `transform.awk` (awk-rs) | Full POSIX awk + GNU extensions |
| `\| cut -d',' -f2` | `transform.awk` | Use awk with field separator |
| `\| sort`, `\| sort -n -r` | `transform.sort` | Full parity |
| `\| sort -u` | `transform.unique` | Full parity |
| `\| wc -l`, `\| wc -c`, `\| wc -w` | `count_lines/bytes/words` | Full parity |
| `VAR=$(cmd)` | `capture: "$VAR"` | Better — typed JSON, not just strings |
| `export VAR=val` | `settings.env` / `let` | Parity |
| `if [ cond ]; then ... fi` | `if` step | Parity for value-based conditions |
| `for x in $LIST; do ... done` | `for_each` step | Better — collect modes, concurrency |
| `cmd &` + `wait` | `mode: "parallel"` | Better — bounded concurrency, structured results |
| `timeout 5 cmd` | `timeout_ms` per step | Better — process group kill |
| `set -e` | `mode: "sequential"` | Parity |
| `\|\| true` (ignore errors) | `mode: "independent"` | Parity |
| `echo "---"` (separators) | JSON structure | Better — no separator needed |

### Covered in v1.1

| Bash Feature | bashli Equivalent | Notes |
|---|---|---|
| `${VAR:-default}` | Interpolation syntax | Default value if undefined/empty |
| `${VAR:?error}` | Interpolation syntax | Abort with message if undefined |
| `${VAR//old/new}` | Interpolation syntax | Replace all in variable |
| `${VAR/old/new}` | Interpolation syntax | Replace first in variable |
| `${VAR%%suffix}` | Interpolation syntax | Strip suffix |
| `${VAR##prefix}` | Interpolation syntax | Strip prefix |
| `${VAR,,}` / `${VAR^^}` | Interpolation syntax | Lowercase / uppercase |
| `while [ cond ]; do ... done` | `While` step | Mandatory max_iterations guard |
| `until [ cond ]; do ... done` | `While` step (inverted condition) | Same step, negate condition |
| `test -f file` | `AssertCondition::FileExists` | Filesystem predicate |
| `test -d dir` | `AssertCondition::DirExists` | Filesystem predicate |
| `grep -q pattern file` | `AssertCondition::FileContains` | File content check |
| `test file1 -nt file2` | `AssertCondition::FileNewer` | File age comparison |
| `$(( 1 + 2 ))` | Math in `let` bindings | `= $A + $B` syntax |
| `case $X in ... esac` | `Case` step | Exact match, glob, regex branches |
| `awk '{print $2}'` | `Awk` transform | Field selection by index |
| `cut -d',' -f2` | `Cut` transform | Delimiter-based field extraction |

### Covered in v1.2

| Bash Feature | bashli Equivalent | Notes |
|---|---|---|
| `trap cleanup EXIT` | `finally` block | Top-level TaskSpec field, always runs |
| `\| tee file` | `tee` field on CmdStep | Simultaneous capture + file write |
| `cmd1 \| cmd2 \| cmd3` | `Pipe` step | True streaming between processes |
| `\| xargs -n 10 cmd` | `batch_size` on ForEach | Batched iteration |
| Diff after write | `diff: true` on WriteStep | Returns unified diff in StepResult |

### Covered in v1.3

| Bash Feature | bashli Equivalent | Notes |
|---|---|---|
| `source script.sh` | `$import` directive | Splice fragment steps into parent TaskSpec |
| `function name() {}` | `Function` definitions + `Call` step | Named reusable step sequences with params |

### Covered in v1.4

| Bash Feature | bashli Equivalent | Notes |
|---|---|---|
| Complex `&`/`wait` patterns | `mode: "dag"` | Dependency-graph-driven parallel execution |

### Intentionally Not Covered

These bash features are not planned for bashli because they are irrelevant to agent-driven execution:

| Bash Feature | Rationale |
|---|---|
| `alias` | Agent doesn't need shortcuts; `$import` and functions cover reuse |
| `history` | Agent has no persistent shell session |
| `PS1` / prompt | No interactive mode |
| `.bashrc` / `.profile` | No login shell concept |
| `select` (interactive menu) | No user interaction during execution |
| Signal traps (SIGUSR1 etc.) | Over-complex; `finally` covers cleanup |
| Coprocesses (`coproc`) | Niche; `Pipe` step + parallel mode cover the use cases |
| `exec` (replace process) | No long-lived process concept |
| `bg` / `fg` / `jobs` | Parallel mode provides structured concurrency |
| `eval` | Security risk; dynamic command construction via interpolation is sufficient |
| `getopts` | bashli is not a shell; CLI args are parsed by clap |
| Here-strings (`<<<`) | `stdin` field covers this |
| Process substitution (`<()`) | Niche; use `Pipe` step or temp file patterns |
| Named pipes (FIFOs) | OS-level construct outside bashli's scope |