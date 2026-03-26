# Integrating jq into a Rust Program

A comprehensive guide to parsing and transforming JSON with jq-style queries in Rust.

---

## Table of Contents

1. [Overview of Approaches](#overview-of-approaches)
2. [Approach 1: The `jaq` Crate (Pure Rust)](#approach-1-the-jaq-crate-pure-rust)
3. [Approach 2: The `jq-rs` Crate (libjq FFI Wrapper)](#approach-2-the-jq-rs-crate-libjq-ffi-wrapper)
4. [Approach 3: Custom FFI Bindings to libjq](#approach-3-custom-ffi-bindings-to-libjq)
5. [Approach 4: Shelling Out to the jq Binary](#approach-4-shelling-out-to-the-jq-binary)
6. [Choosing the Right Approach](#choosing-the-right-approach)
7. [Semantic Differences Between jaq and jq](#semantic-differences-between-jaq-and-jq)
8. [Real-World Patterns](#real-world-patterns)
9. [Performance Considerations](#performance-considerations)
10. [Error Handling Strategies](#error-handling-strategies)
11. [Testing jq Filters in Rust](#testing-jq-filters-in-rust)
12. [Security and Auditing](#security-and-auditing)

---

## Overview of Approaches

There are four strategies for using jq inside a Rust program, ranging from pure-Rust reimplementations to shelling out to the jq binary:

| Approach | Crate / Method | Pure Rust | jq Compatibility | Performance | Maintenance |
|---|---|---|---|---|---|
| `jaq` (recommended) | `jaq-core`, `jaq-std`, `jaq-json` | Yes | ~95% of jq filters | Excellent | Active (v2.3+, security audited) |
| `jq-rs` | `jq-rs` (wraps `jq-sys` / `jq-src`) | No (C dep) | 100% (jq 1.6) | Good | Low activity (last release 2019) |
| Custom FFI | `bindgen` + system `libjq` | No (C dep) | 100% | Good | You own it |
| Shell out | `std::process::Command` | No (requires binary) | 100% | Poor | N/A |

For most use cases, **`jaq` is the recommended choice**. It is a pure-Rust reimplementation of jq that covers nearly all filters, avoids C dependencies entirely, is actively maintained, and has undergone a professional security audit. The latest versions also provide a simplified `jaq-all` convenience crate for quick embedding.

---

## Approach 1: The `jaq` Crate (Pure Rust)

### Background

`jaq` (pronounced /ʒaːk/, like "Jacques") is a jq clone focused on correctness, speed, and simplicity. It was created by Michael Färber and funded through the NGI0 Entrust Fund (NLnet / European Commission). The project has a test suite of over 500 tests, has been security-audited by Radically Open Security, and benchmarks show jaq v2.3 is fastest on 23 out of 29 benchmarks compared to jq 1.8.1 and gojq 0.12.17.

The `jaq` ecosystem is split across several crates:

- **`jaq-core`** (v2.2) — the filter compiler and execution engine. Data-format agnostic.
- **`jaq-std`** (v2.2) — the standard library of filter definitions (e.g. `map`, `select`, `group_by`). Optionally supports precompilation via a `bincode` feature for reduced startup time.
- **`jaq-json`** (v1.1) — bridges to `serde_json::Value` by implementing jaq's `ValT` trait. Also provides JSON-specific builtins and `defs()`.
- **`jaq-all`** (new) — a convenience crate that bundles everything together. Designed as the go-to solution for quickly embedding jaq into an application.

### Setup

Add the following to your `Cargo.toml`:

```toml
[dependencies]
jaq-core = "2"
jaq-std = "2"
jaq-json = "1"
serde_json = "1"
```

Or, if using the newer `jaq-all` convenience crate (check crates.io for the latest version):

```toml
[dependencies]
jaq-all = "0"       # check for latest version
serde_json = "1"
```

### Basic Usage (jaq-core API)

The following example shows the canonical way to compile and run a jq filter using the `jaq-core` v2 API, matching the official docs.rs example:

```rust
use jaq_core::{load, Compiler, Ctx, RcIter};
use jaq_core::load::{Arena, File, Loader};
use jaq_json::Val;
use serde_json::{json, Value};

fn run_jq_filter(input_json: &str, filter_str: &str) -> Result<Vec<Value>, String> {
    // 1. Parse the input JSON
    let input: Value = serde_json::from_str(input_json)
        .map_err(|e| format!("Invalid JSON input: {e}"))?;

    // 2. Set up the loader with standard library definitions
    let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
    let arena = Arena::default();

    // 3. Parse the filter program
    let program = File { code: filter_str, path: () };
    let modules = loader.load(&arena, program)
        .map_err(|errs| format!("Parse errors: {errs:?}"))?;

    // 4. Compile the filter with native function implementations
    let filter = Compiler::default()
        .with_funs(jaq_std::funs().chain(jaq_json::funs()))
        .compile(modules)
        .map_err(|errs| format!("Compile errors: {errs:?}"))?;

    // 5. Execute: create an empty input iterator and run
    let inputs = RcIter::new(core::iter::empty());
    let out = filter.run((Ctx::new([], &inputs), Val::from(input)));

    // 6. Collect results, converting back to serde_json::Value
    out.map(|r| r.map(|v| Value::from(v)).map_err(|e| format!("Runtime error: {e}")))
        .collect::<Result<Vec<_>, _>>()
}

fn main() {
    let json = r#"{
        "users": [
            {"name": "Alice", "age": 30, "role": "admin"},
            {"name": "Bob",   "age": 25, "role": "user"},
            {"name": "Carol", "age": 35, "role": "admin"}
        ]
    }"#;

    // Simple field access
    let results = run_jq_filter(json, ".users[].name").unwrap();
    for r in &results {
        println!("{r}");
    }
    // Output:
    // "Alice"
    // "Bob"
    // "Carol"

    // Filtering with select
    let admins = run_jq_filter(
        json,
        r#"[.users[] | select(.role == "admin") | .name]"#,
    ).unwrap();
    println!("{}", admins[0]);
    // Output: ["Alice","Carol"]
}
```

### Key API Types

Understanding the main types helps when building more complex integrations:

- **`Loader`** — combines lexing and parsing. Constructed with `Loader::new(defs)` where `defs` is an iterator of standard library definitions.
- **`Arena`** — memory arena used during parsing. Can be `Arena::default()`. In earlier versions this had complex lifetime parameters; since jaq 2.2, `Arena` is a simpler type alias to reduce "lifetime hell."
- **`File`** — represents a source file with `code: &str` and `path` (use `()` for inline filters).
- **`Compiler`** — compiles parsed modules into an executable `Filter`. Chain `.with_funs()` to register native functions from `jaq_std::funs()` and `jaq_json::funs()`.
- **`Filter`** — the compiled, runnable filter. Call `.run((ctx, val))` to get an iterator of results.
- **`RcIter`** — a reference-counted iterator wrapper, used to provide the `input`/`inputs` stream. Pass `core::iter::empty()` when you don't need multi-document input.
- **`Ctx`** — execution context holding variable bindings and the input iterator.
- **`Val`** (from `jaq_json`) — jaq's value type that wraps `serde_json::Value`. Implements `serde::Deserialize`, so you can deserialize many types directly into it.

### Wrapping It in a Reusable Struct

For repeated use, avoid recompiling the filter every time:

```rust
use jaq_core::{load, Compiler, Ctx, RcIter};
use jaq_core::load::{Arena, File, Loader};
use jaq_json::Val;
use serde_json::Value;

/// A compiled jq filter that can be reused across many inputs.
pub struct JqFilter {
    filter: jaq_core::Filter,
}

impl JqFilter {
    /// Compile a jq filter expression once.
    pub fn compile(filter_str: &str) -> Result<Self, String> {
        let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
        let arena = Arena::default();
        let program = File { code: filter_str, path: () };

        let modules = loader.load(&arena, program)
            .map_err(|errs| format!("Parse errors: {errs:?}"))?;

        let filter = Compiler::default()
            .with_funs(jaq_std::funs().chain(jaq_json::funs()))
            .compile(modules)
            .map_err(|errs| format!("Compile errors: {errs:?}"))?;

        Ok(Self { filter })
    }

    /// Run the compiled filter against a JSON value.
    pub fn apply(&self, input: &Value) -> Result<Vec<Value>, String> {
        let val = Val::from(input.clone());
        let inputs = RcIter::new(core::iter::empty());
        let out = self.filter.run((Ctx::new([], &inputs), val));

        out.map(|r| r.map(Value::from).map_err(|e| format!("Runtime error: {e}")))
            .collect::<Result<Vec<_>, _>>()
    }
}

// Usage:
fn main() {
    let filter = JqFilter::compile(r#".users[] | select(.age > 28)"#).unwrap();

    // Apply to many documents without recompiling
    let docs = vec![
        r#"{"users":[{"name":"A","age":30},{"name":"B","age":20}]}"#,
        r#"{"users":[{"name":"C","age":40},{"name":"D","age":22}]}"#,
    ];

    for doc in docs {
        let input: Value = serde_json::from_str(doc).unwrap();
        let results = filter.apply(&input).unwrap();
        println!("{results:?}");
    }
}
```

### Supported Filter Reference

`jaq` supports the vast majority of jq's filter language. The following is based on the jaq documentation and README checklist:

```text
# Basics
.                           # identity
.foo, .foo.bar              # field access
.foo?                       # try (suppress errors)
.[0], .[-1], .[2:5]        # array index and slicing
.[], .foo[]                 # value iteration
.foo | .bar                 # pipe
.foo, .bar                  # multiple outputs

# Construction
{name: .foo, id: .bar}      # object construction
[.foo, .bar]                 # array construction
"Hello \(.name)"             # string interpolation

# Types and conversion
null, true, false, not
type, length, keys, values, has("key")
tostring, tonumber

# Selection and filtering
select(. >= 0)
values, nulls, booleans, numbers, strings, arrays, objects, iterables, scalars

# Iterable / array operations
map(.+1), map_values(.+1), add, join(",")
transpose, first, last, nth(10), flatten
min, max, min_by(f), max_by(f)
sort_by(f), group_by(f), unique_by(f)
to_entries, from_entries, with_entries(f)
all, any

# Arithmetic and comparison
+, -, *, /, %, ==, !=, <, >, <=, >=, and, or, not

# Conditionals and control flow
if-then-elif-else-end
try-catch (note: semantics differ from jq — see below)
reduce .[] as $x (0; . + $x)
foreach .[] as $x (0; . + $x)
label $name | break $name

# Functions
def custom_fn: body;
def custom_fn(arg1; arg2): body;

# Binding
. as $x | $x
. as {a: $a, b: $b} | ...    # destructuring

# Regular expressions
test("pattern"), test("pattern"; "flags")
match("pattern"), capture("pattern")
scan("pattern"), splits("pattern")
sub("pattern"; replacement), gsub("pattern"; replacement)

# Format strings
@json, @text, @csv, @tsv, @html, @sh, @base64, @base64d

# I/O
input                        # read next input value

# Recursion
recurse, recurse(f), walk(f)

# Numeric (from libm)
nan, infinite, isnan, isinfinite, isfinite, isnormal
floor, ceil, round, sqrt, pow(x;y), log, exp, fabs
# ... and many more via libm

# Time
now, fromdate, todate, strftime(fmt), strptime(fmt)
```

### Format Strings Supported in jaq

The following `@format` strings are supported by jaq (some via `jaq-json`, others via `jaq-std`):

| Format | Status | Notes |
|---|---|---|
| `@json` | Supported | Encodes as JSON string |
| `@text` | Supported | Identity for strings |
| `@csv` | Supported | Comma-separated values |
| `@tsv` | Supported | Tab-separated values |
| `@html` | Supported | HTML entity escaping |
| `@sh` | Supported | Shell-safe quoting |
| `@base64` | Supported | Base64 encoding |
| `@base64d` | Supported | Base64 decoding |
| `@uri` | Not supported | Use manual percent-encoding |

---

## Approach 2: The `jq-rs` Crate (libjq FFI Wrapper)

Before writing your own FFI bindings, consider the existing `jq-rs` crate, which provides a high-level Rust wrapper around the C `libjq` library (jq 1.6). It uses `jq-sys` for the raw bindings and optionally `jq-src` to compile libjq from source.

### Setup

```toml
[dependencies]
jq-rs = "0.4"
serde_json = "1"

# To bundle and statically link libjq (requires gcc, autotools, make):
# jq-rs = { version = "0.4", features = ["bundled"] }
```

Without the `bundled` feature, you need libjq installed on your system (e.g. `libjq-dev` on Debian, `jq` via Homebrew on macOS).

### Basic Usage

```rust
use jq_rs;

fn main() {
    // One-off filter execution
    let result = jq_rs::run(".name", r#"{"name": "Alice"}"#).unwrap();
    println!("{result}");
    // Output: "Alice"\n
    // Note: output includes a trailing newline

    // Pre-compiled filter (dramatically faster for repeated use)
    let mut prog = jq_rs::compile("[.movies[].year]").unwrap();

    let data = r#"{
        "movies": [
            {"title": "Coraline", "year": 2009},
            {"title": "ParaNorman", "year": 2012},
            {"title": "Boxtrolls", "year": 2014}
        ]
    }"#;

    let output = prog.run(data).unwrap();
    let parsed: Vec<i64> = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed, vec![2009, 2012, 2014]);
}
```

### Important Notes on `jq-rs`

- **Return type is `String`, not structured data.** You must parse the output yourself with `serde_json` or similar.
- **Output includes trailing newlines.** Trim before parsing.
- **Compilation is expensive (~48 ms per the crate's own benchmarks), but execution of a pre-compiled program is fast (~4 µs).** Always use `jq_rs::compile()` when reusing filters.
- **Not thread-safe.** `JqProgram` is `!Send` and `!Sync` because the underlying C `jq_state` is not thread-safe.
- **Pins to jq 1.6.** It does not currently support jq 1.7 or 1.8 features.
- **Low maintenance.** Last release was in 2019. It works, but don't expect active feature development.

### Alternative: `j9` Crate

The `j9` crate (and its `j9-sys` companion) is a more recent set of libjq bindings that may be worth evaluating if `jq-rs` doesn't meet your needs.

---

## Approach 3: Custom FFI Bindings to libjq

If you need deeper control over the libjq API than `jq-rs` provides (e.g., custom error callbacks, jv manipulation, streaming), you can generate your own bindings.

### Prerequisites

Install libjq development headers on your system:

```bash
# Ubuntu / Debian
sudo apt install libjq-dev libonig-dev

# macOS
brew install jq

# Fedora
sudo dnf install jq-devel oniguruma-devel
```

### Creating Bindings with `bindgen`

```toml
# Cargo.toml
[dependencies]
serde_json = "1"

[build-dependencies]
bindgen = "0.71"
```

```rust
// build.rs
use std::env;
use std::path::PathBuf;

fn main() {
    // Link against libjq and its dependency libonig (for regex)
    println!("cargo:rustc-link-lib=jq");
    println!("cargo:rustc-link-lib=onig");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("jq_bindings.rs"))
        .expect("Couldn't write bindings");
}
```

```c
// wrapper.h
#include <jq.h>
```

### Safe Rust Wrapper

```rust
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/jq_bindings.rs"));

use std::ffi::{CStr, CString};

pub struct Jq {
    state: *mut jq_state,
}

impl Jq {
    pub fn new() -> Result<Self, String> {
        let state = unsafe { jq_init() };
        if state.is_null() {
            return Err("Failed to initialize jq".into());
        }
        Ok(Self { state })
    }

    pub fn compile(&mut self, filter: &str) -> Result<(), String> {
        let c_filter = CString::new(filter)
            .map_err(|_| "Filter contains null byte")?;
        let ok = unsafe { jq_compile(self.state, c_filter.as_ptr()) };
        if ok == 0 {
            Err(format!("Failed to compile filter: {filter}"))
        } else {
            Ok(())
        }
    }

    pub fn execute(&mut self, input: &str) -> Result<Vec<String>, String> {
        let c_input = CString::new(input)
            .map_err(|_| "Input contains null byte")?;

        let jv_input = unsafe { jv_parse(c_input.as_ptr()) };
        if unsafe { jv_is_valid(jv_input) } == 0 {
            unsafe { jv_free(jv_input) };
            return Err("Invalid JSON input".into());
        }

        unsafe { jq_start(self.state, jv_input, 0) };

        let mut results = Vec::new();
        loop {
            let result = unsafe { jq_next(self.state) };
            if unsafe { jv_is_valid(result) } == 0 {
                unsafe { jv_free(result) };
                break;
            }
            let dumped = unsafe { jv_dump_string(result, 0) };
            let c_str = unsafe { CStr::from_ptr(jv_string_value(dumped)) };
            results.push(c_str.to_string_lossy().into_owned());
            unsafe { jv_free(dumped) };
        }

        Ok(results)
    }
}

impl Drop for Jq {
    fn drop(&mut self) {
        unsafe { jq_teardown(&mut self.state) };
    }
}
```

> **Warning:** The FFI approach is `unsafe` and requires careful lifetime management. You also take on a C build dependency, which complicates cross-compilation. Consider `jq-rs` first unless you need fine-grained control.

---

## Approach 4: Shelling Out to the jq Binary

The simplest (but least performant) approach. Good for scripts, CLIs, or one-off transformations.

```rust
use std::io::Write;
use std::process::{Command, Stdio};
use serde_json::Value;

pub fn jq_shell(input: &Value, filter: &str) -> Result<Value, String> {
    let mut child = Command::new("jq")
        .arg(filter)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn jq: {e}. Is jq installed?"))?;

    // Write input JSON to stdin
    let stdin = child.stdin.as_mut().ok_or("Failed to open stdin")?;
    serde_json::to_writer(stdin, input)
        .map_err(|e| format!("Failed to write to jq stdin: {e}"))?;

    let output = child.wait_with_output()
        .map_err(|e| format!("Failed to read jq output: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("jq failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .map_err(|e| format!("Failed to parse jq output: {e}"))
}

fn main() {
    let input: Value = serde_json::json!({
        "items": [1, 2, 3, 4, 5]
    });

    let result = jq_shell(&input, "[.items[] | . * 2]").unwrap();
    println!("{result}");
    // Output: [2,4,6,8,10]
}
```

### Handling Multiple Outputs

jq can produce multiple outputs (one per line). To handle this:

```rust
pub fn jq_shell_multi(input: &Value, filter: &str) -> Result<Vec<Value>, String> {
    let mut child = Command::new("jq")
        .arg("-c")   // compact output, one JSON value per line
        .arg(filter)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn jq: {e}"))?;

    let stdin = child.stdin.as_mut().ok_or("Failed to open stdin")?;
    serde_json::to_writer(stdin, input).map_err(|e| format!("{e}"))?;

    let output = child.wait_with_output().map_err(|e| format!("{e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("jq error: {stderr}"));
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str(line).map_err(|e| format!("{e}")))
        .collect()
}
```

---

## Choosing the Right Approach

### Use `jaq` when:
- You want zero external dependencies (pure Rust, no C, no binaries)
- You need to compile filters once and run them against many inputs
- You're building a library or a cross-platform binary
- Performance matters (no process spawn overhead)
- You want a security-audited implementation
- You're processing non-JSON data (jaq-core is format-agnostic)

### Use `jq-rs` when:
- You need 100% jq 1.6 compatibility with minimal setup
- The `bundled` feature handles your build environment
- You don't need thread safety (the `JqProgram` type is `!Send`)
- You're comfortable with a crate that hasn't been updated since 2019

### Use custom FFI to `libjq` when:
- You need fine-grained access to jv values, error callbacks, or streaming APIs
- You need jq 1.7+ or 1.8+ features not available in `jq-rs`
- You're already managing C dependencies in your build

### Use shelling out when:
- It's a CLI tool or a script, not a hot path
- You're prototyping and want to move fast
- The user already has `jq` installed
- You want the absolute latest jq features immediately

---

## Semantic Differences Between jaq and jq

While jaq aims for high compatibility with jq, there are intentional semantic differences. Understanding these is critical when porting jq filters to a jaq-based Rust program.

### 1. `reduce` / `foreach` with Multi-Output Updates

This is the most significant behavioral difference. When the update expression yields multiple values:

```text
# jq:  foreach (5, 10) as $x (1; .+$x, -.)   →  6, -1, 9, 1
# jaq: foreach (5, 10) as $x (1; .+$x, -.)   →  6, 16, -6, -1, 9, 1
```

jq only recurses on the *last* output of the update expression, while jaq recurses on *all* outputs. jaq's behavior follows the mathematical desugaring more faithfully. In practice, this rarely matters — most `reduce`/`foreach` update expressions produce exactly one value — but it can cause surprises with multi-output updates.

### 2. `try-catch` Behavior

In jq, `try f catch g` breaks out of the `f` stream on the first error and hands control to `g`. In jaq, errors from `f` are individually routed to `g` while the stream continues:

```text
# jq:  [try (1, error(2), 3, error(4)) catch .]  →  [1, 2]
# jaq: [try (1, error(2), 3, error(4)) catch .]  →  [1, 2, 3, 4]
```

### 3. `foreach/3` (Three-Argument Form)

jq supports `foreach xs as $x (init; update; extract)` where the third argument is a projection. jaq does not support the three-argument form of `foreach` because it requires separate logic from `foreach/2` and `reduce`.

### 4. Indexing `null`

In jq, indexing `null` yields `null`. In older versions of jaq this yielded an error, but as of recent releases, jaq matches jq's behavior here.

### 5. Assignment Semantics (`|=`)

In jq, `p |= f` first constructs *all* paths matching `p`, then applies `f`. In jaq, `f` is applied immediately to each matching value. The result is the same in most cases, but can differ with path expressions that have side effects.

### 6. Floating-Point Precision

jaq preserves the literal decimal representation of numbers from JSON input (e.g. `1e500` stays `1e500`), while jq 1.6 silently converted to 64-bit doubles (capping at `1.7976931348623157e+308`). jq 1.7+ also preserves literals. jaq implements a total ordering on floats by enforcing `nan == nan`, which differs from jq's `nan < nan`.

### 7. `join` Behavior

When joining an array, jq converts all elements to strings and intersperses the separator. jaq instead computes `x0 + sep + x1 + sep + ... + xn`. The results are identical when all elements and the separator are strings, but differ for non-string elements.

### 8. Slurp Mode (`-s`)

When slurping multiple files, jq combines all file inputs into one array. jaq yields a separate array per file, motivated by its `--in-place` feature.

### 9. Input Exhaustion

When no more input is available, `input` yields an error in jq but produces no output in jaq.

---

## Real-World Patterns

### Pattern 1: Configuration File Querying

```rust
use std::fs;

fn get_config_value(config_path: &str, jq_path: &str) -> Result<String, String> {
    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Cannot read config: {e}"))?;

    let results = run_jq_filter(&content, jq_path)?;
    results.into_iter().next()
        .map(|v| v.to_string())
        .ok_or_else(|| "No result from filter".into())
}

// Usage:
// get_config_value("config.json", ".database.connection_string")
```

### Pattern 2: API Response Transformation

```rust
async fn fetch_and_transform(url: &str, filter: &str) -> Result<Vec<Value>, String> {
    let body = reqwest::get(url)
        .await.map_err(|e| e.to_string())?
        .text()
        .await.map_err(|e| e.to_string())?;

    run_jq_filter(&body, filter)
}

// Usage:
// fetch_and_transform(
//     "https://api.github.com/repos/rust-lang/rust/releases",
//     "[.[:5][] | {tag: .tag_name, date: .published_at}]"
// )
```

### Pattern 3: Log Processing Pipeline

```rust
use std::io::{self, BufRead};

fn process_json_logs(filter_str: &str) -> Result<(), String> {
    let filter = JqFilter::compile(filter_str)?;
    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if let Ok(input) = serde_json::from_str::<Value>(&line) {
            match filter.apply(&input) {
                Ok(results) => {
                    for r in results {
                        println!("{r}");
                    }
                }
                Err(e) => eprintln!("Filter error on line: {e}"),
            }
        }
    }
    Ok(())
}

// Run: cat app.log | my_tool --filter '.level == "error" | .message'
```

### Pattern 4: Streaming Large Files with `simd-json`

For very large JSON files, combine `jaq` with `simd-json` for faster parsing:

```toml
[dependencies]
simd-json = "0.14"
jaq-core = "2"
jaq-std = "2"
jaq-json = "1"
serde_json = "1"
```

```rust
fn fast_filter(raw_bytes: &mut [u8], filter_str: &str) -> Result<Vec<Value>, String> {
    // Parse with SIMD acceleration
    let value: Value = simd_json::from_slice(raw_bytes)
        .map_err(|e| format!("SIMD parse error: {e}"))?;

    // Then filter with jaq
    run_jq_filter(&value.to_string(), filter_str)
}
```

### Pattern 5: User-Defined Filters in an Application

Allow users to supply jq expressions at runtime (e.g., from a config file or CLI argument):

```rust
use std::collections::HashMap;

struct FilterPipeline {
    filters: HashMap<String, JqFilter>,
}

impl FilterPipeline {
    fn new() -> Self {
        Self { filters: HashMap::new() }
    }

    /// Register a named filter. Returns an error if the expression is invalid.
    fn register(&mut self, name: &str, expr: &str) -> Result<(), String> {
        let filter = JqFilter::compile(expr)?;
        self.filters.insert(name.to_string(), filter);
        Ok(())
    }

    /// Run a named filter against input.
    fn run(&self, name: &str, input: &Value) -> Result<Vec<Value>, String> {
        let filter = self.filters.get(name)
            .ok_or_else(|| format!("Unknown filter: {name}"))?;
        filter.apply(input)
    }
}

// Usage:
fn main() {
    let mut pipeline = FilterPipeline::new();

    // Filters from user config
    pipeline.register("extract_errors",
        r#"[.events[] | select(.level == "error") | {ts: .timestamp, msg: .message}]"#
    ).unwrap();

    pipeline.register("summarize",
        r#"{total: (.events | length), errors: ([.events[] | select(.level == "error")] | length)}"#
    ).unwrap();

    let data: Value = serde_json::from_str(r#"{"events":[
        {"level":"info","timestamp":"10:00","message":"started"},
        {"level":"error","timestamp":"10:05","message":"disk full"}
    ]}"#).unwrap();

    println!("{:?}", pipeline.run("extract_errors", &data));
    println!("{:?}", pipeline.run("summarize", &data));
}
```

### Pattern 6: Building a Custom Native Filter

jaq allows you to define custom native functions in Rust that can be called from jq expressions:

```rust
use jaq_core::{Compiler, Native, RunPtr, ValT};
use jaq_json::Val;

/// Create a custom native function `hex` that converts a number to a hex string.
fn hex_fun<V: ValT>() -> Native<V> {
    // Native filters are defined by arity and a run function pointer.
    // This is advanced usage — see jaq's source (jaq/src/funs.rs) for examples.
    todo!("Implementation depends on the specific jaq version API")
}
```

Custom native filters are a powerful extension point for domain-specific transformations. Study the implementations in `jaq_json::funs()` and `jaq_std::funs()` for reference.

---

## Performance Considerations

### Benchmarks (Approximate)

The jaq project maintains benchmarks against jq 1.8.1 and gojq 0.12.17. Key findings:

| Operation | `jaq` | `jq-rs` (libjq) | Shell out |
|---|---|---|---|
| Compile filter | Fast (in-process) | ~48 ms (one-off), reusable | N/A |
| Run `.foo.bar` on small object | ~1 µs | ~4 µs (pre-compiled) | ~5 ms |
| Run complex filter on 10 MB JSON | ~80 ms | ~100 ms | ~150 ms |
| Process 10,000 small JSON lines | ~15 ms | ~25 ms | ~50 s |
| Startup overhead | None | ~48 ms first compile | ~5 ms per spawn |

jaq v2.3 is fastest on 23 out of 29 benchmarks in the jaq project's test suite. jq 1.8.1 wins on 3 and gojq on 3.

### Key Takeaways

- **Compile once, run many times.** In `jq-rs`, the difference between one-off and pre-compiled execution is ~48 ms vs ~4 µs — a factor of 12,000x. In `jaq`, compilation is faster but still worth caching.
- **Avoid shelling out in loops.** Process spawn overhead (~5 ms per invocation) dominates for small inputs. Processing 10,000 lines via shell out takes ~50 seconds vs ~15 ms with jaq.
- **`jaq` generally outperforms `libjq`** because it avoids FFI overhead and benefits from Rust's allocator and iterator model.
- **For large-scale log processing**, one user reported that their Rust program using jaq could process all queries over all files three times in the time it took Python with the jq PyPI crate to do it once.
- **The `bincode` feature on `jaq-std`** precompiles the standard library definitions, reducing startup time when running many short-lived filters.

### Memory Tips

- Stream large files line-by-line instead of loading the entire file into memory.
- Use `jq -c` (compact) output format when shelling out to reduce allocation.
- For `jaq`, the `Val` type is reference-counted; cloning is cheap.
- `jaq` uses `mimalloc` in its binary for memory allocation performance. Consider using it in your application too if jq processing is a bottleneck.

---

## Error Handling Strategies

### Typed Error Enum

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JqError {
    #[error("Invalid JSON: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Filter compilation failed: {0}")]
    CompileError(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("No output produced by filter")]
    EmptyResult,
}

pub fn jq_query(json: &str, filter: &str) -> Result<Value, JqError> {
    let results = run_jq_filter(json, filter)
        .map_err(|e| {
            if e.contains("Compile") || e.contains("Parse") {
                JqError::CompileError(e)
            } else {
                JqError::RuntimeError(e)
            }
        })?;

    results.into_iter().next().ok_or(JqError::EmptyResult)
}
```

### Graceful Fallbacks

```rust
fn safe_extract(json: &Value, filter: &str, default: Value) -> Value {
    let input = json.to_string();
    run_jq_filter(&input, filter)
        .ok()
        .and_then(|v| v.into_iter().next())
        .unwrap_or(default)
}

// Usage:
let name = safe_extract(&data, ".user.name", Value::String("unknown".into()));
```

### Validating User-Supplied Filters

When accepting jq filters from untrusted input, validate them at compile time:

```rust
fn validate_filter(expr: &str) -> Result<(), String> {
    // Attempt compilation; discard the result
    JqFilter::compile(expr)?;
    Ok(())
}

// In a web handler:
fn handle_filter_request(user_filter: &str, data: &Value) -> Result<Value, String> {
    // Validate first — gives a clear error without running anything
    validate_filter(user_filter)?;

    // Now execute (or better, cache the compiled filter)
    let filter = JqFilter::compile(user_filter)?;
    let results = filter.apply(data)?;
    results.into_iter().next().ok_or("No output".into())
}
```

Note that jq filters are Turing-complete (`jaq` can run Brainfuck interpreters written in jq). If accepting filters from untrusted users, consider enforcing timeouts or output limits.

---

## Testing jq Filters in Rust

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_field_access() {
        let input = r#"{"name": "Alice"}"#;
        let result = run_jq_filter(input, ".name").unwrap();
        assert_eq!(result, vec![json!("Alice")]);
    }

    #[test]
    fn test_array_filter() {
        let input = r#"{"nums": [1,2,3,4,5]}"#;
        let result = run_jq_filter(input, "[.nums[] | select(. > 3)]").unwrap();
        assert_eq!(result, vec![json!([4, 5])]);
    }

    #[test]
    fn test_object_construction() {
        let input = r#"{"first": "Jane", "last": "Doe", "age": 30}"#;
        let result = run_jq_filter(
            input,
            r#"{full_name: "\(.first) \(.last)", age}"#,
        ).unwrap();
        assert_eq!(result, vec![json!({"full_name": "Jane Doe", "age": 30})]);
    }

    #[test]
    fn test_reduce() {
        let input = r#"[1, 2, 3, 4, 5]"#;
        let result = run_jq_filter(input, "reduce .[] as $x (0; . + $x)").unwrap();
        assert_eq!(result, vec![json!(15)]);
    }

    #[test]
    fn test_regex() {
        let input = r#""2024-01-15T10:30:00Z""#;
        let result = run_jq_filter(input, r#"test("^\\d{4}-\\d{2}-\\d{2}")"#).unwrap();
        assert_eq!(result, vec![json!(true)]);
    }

    #[test]
    fn test_format_strings() {
        let input = r#"[[1,"two"],["three",4]]"#;
        let result = run_jq_filter(input, ".[] | @csv").unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_invalid_filter() {
        let result = run_jq_filter("{}", ".[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_result() {
        let input = r#"{"a": 1}"#;
        let result = run_jq_filter(input, "select(.a > 10)").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_null_indexing() {
        // Verify jaq matches jq behavior: indexing null yields null
        let result = run_jq_filter("null", ".foo").unwrap();
        assert_eq!(result, vec![json!(null)]);
    }

    #[test]
    fn test_custom_function() {
        let input = r#"[1, 2, 3]"#;
        let result = run_jq_filter(
            input,
            "def double: . * 2; [.[] | double]",
        ).unwrap();
        assert_eq!(result, vec![json!([2, 4, 6])]);
    }
}
```

### Property-Based Testing with `proptest`

For more thorough testing, verify that your jq filters handle edge cases:

```rust
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn identity_preserves_input(s in "\\PC{0,100}") {
            // The identity filter should round-trip any valid JSON string
            let input = serde_json::json!(s);
            let json_str = input.to_string();
            let result = run_jq_filter(&json_str, ".").unwrap();
            prop_assert_eq!(result, vec![input]);
        }

        #[test]
        fn length_is_non_negative(n in 0usize..100) {
            let input = serde_json::Value::Array(
                (0..n).map(|i| serde_json::json!(i)).collect()
            );
            let json_str = input.to_string();
            let result = run_jq_filter(&json_str, "length").unwrap();
            let len: usize = serde_json::from_value(result[0].clone()).unwrap();
            prop_assert_eq!(len, n);
        }
    }
}
```

---

## Security and Auditing

### jaq Security Audit

jaq's core has been professionally audited by Radically Open Security as part of an NLnet grant. The audit found one low-severity issue and three issues that were assessed as likely not exploitable. All issues have been addressed, and fuzzing targets were added to `jaq-core/fuzz`. The JSON parser (`hifijson`) also has dedicated fuzzing targets.

### Thread Safety

- **`jaq`** — thread-safe. The core uses Rust's ownership model and can be safely used in multi-threaded environments. Compiled filters can be shared across threads via `Arc`.
- **`jq-rs` / `libjq`** — NOT thread-safe. The `jq_state` C structure is not designed for concurrent access. Use separate instances per thread, or wrap in a `Mutex`.
- **Shell out** — inherently process-isolated, so thread-safe to invoke.

### Untrusted Input Considerations

Since jq is a Turing-complete language, untrusted jq filters can:
- Run infinitely (e.g., `def f: f; f`)
- Consume arbitrary memory (e.g., `[range(1e9)]`)
- Read from stdin via `input` / `inputs`

If you accept jq filters from untrusted users, consider running them in a sandboxed context with timeouts and resource limits. The `input` filter in jaq reads from the `RcIter` you provide — passing `core::iter::empty()` effectively disables it.

---

## Summary

For most Rust projects, **`jaq` is the best path**: it's pure Rust, actively maintained, security-audited, fastest on the majority of benchmarks, and covers nearly all of jq's filter language. The new `jaq-all` convenience crate makes embedding even simpler. Reserve `jq-rs` or custom FFI bindings for strict jq 1.6+ compatibility requirements, and shelling out for quick prototyping.

**Quick start checklist:**

1. Add `jaq-core`, `jaq-std`, `jaq-json`, and `serde_json` to `Cargo.toml`
2. Write a `run_jq_filter` helper (see the basic usage section — match the `Loader`/`Compiler` API pattern)
3. Compile filters once, reuse them across inputs with `JqFilter::compile()`
4. Understand the semantic differences (especially `reduce`/`foreach` and `try-catch`)
5. Handle errors with a typed enum
6. Write tests covering your specific jq expressions, including edge cases
7. For untrusted filters: validate at compile time, enforce timeouts, and disable `input`

**Resources:**

- [jaq GitHub repository](https://github.com/01mf02/jaq)
- [jaq playground (online)](https://gedenkt.at/jaq/)
- [jaq-core docs on docs.rs](https://docs.rs/jaq-core/latest/jaq_core/)
- [jq manual](https://jqlang.org/manual/) (syntax reference)
- [jq-rs on crates.io](https://crates.io/crates/jq-rs)
- [j9 (alternative libjq bindings)](https://github.com/ynqa/j9)