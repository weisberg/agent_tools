# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is bashli

bashli is a Rust CLI that accepts a JSON task specification, executes structured shell workflows, and returns machine-readable JSON results. It replaces raw bash invocations in agentic workflows — eliminating parser-triggered safety prompts while adding variable capture, transforms, extraction, token-budget truncation, and structured I/O routing.

The full product spec lives in `bashli-spec-final.md`. The implementation plan is in `PLAN.md`.

## Project Status

Greenfield — the spec and plan exist but the Rust workspace has not been scaffolded yet. Implementation follows the phased plan in `PLAN.md` (Phase 0 through Phase 8).

## Build & Test Commands

```bash
# Format check
cargo fmt --check

# Lint (deny warnings)
cargo clippy --workspace --all-targets -- -D warnings

# Run all tests
cargo test --workspace

# Run tests for a single crate
cargo test -p bashli-core
cargo test -p bashli-vars

# Run a specific test
cargo test -p bashli-core -- test_name

# Verify workspace compiles
cargo metadata
```

## Architecture

### Crate Workspace

bashli is a Cargo workspace under `crates/` with a strict layered dependency DAG — no cycles, no lateral dependencies between peers:

```
bashli-cli → bashli-engine → bashli-steps → {bashli-runner, bashli-transforms, bashli-extract, bashli-budget} → bashli-vars → bashli-core
```

The three thin wrapper crates (`bashli-jq`, `bashli-sed`, `bashli-awk`) have **zero internal bashli deps** — they only wrap third-party crates (`jaq-*`, `sedregex`, `awk-rs`).

### Key crate responsibilities

- **bashli-core**: Pure data types (TaskSpec, StepResult, errors, enums). Zero logic, zero I/O. All other crates depend on this.
- **bashli-vars**: Variable store, `$VAR` interpolation engine, JSON path resolution, shell escaping. All variable values must be escaped via `escape.rs` before shell interpolation.
- **bashli-transforms / bashli-extract**: Registry-driven. Each built-in implements `TransformFn` or `ExtractorFn` trait and registers with the corresponding registry.
- **bashli-runner**: Subprocess execution only — spawn, timeout, process group kill. No variable or transform logic.
- **bashli-budget**: Token estimation, allocation strategies, smart truncation. Self-contained.
- **bashli-steps**: Integration layer — wires vars, transforms, extraction, runner, and budget into step execution via `StepExecutor` trait + `StepRegistry`.
- **bashli-engine**: Thin orchestration loop. Implements Sequential and Independent execution modes (v1.0). Processes `let_vars` and system variables before step dispatch.
- **bashli-cli**: Argument parsing (clap), input sources (inline JSON, file, stdin), output formatting, exit code mapping (0=success, 1=task failure, 2=parse/validation error).

### CmdStep execution pipeline (critical path)

This is the data flow through a command step — get this wrong and capture/extraction/budget will diverge:

1. Interpolate `cmd`, `cwd`, `env`, `stdin`
2. Run command via bashli-runner
3. Collect raw stdout/stderr
4. Apply I/O routing (merge/discard/file/tee)
5. Apply transforms → "transformed output"
6. Simultaneously: store into capture variable AND run extractions (parallel consumers of same data)
7. Apply per-step `limit` to StepResult output
8. Charge against global token budget
9. Generate StepResult
10. If exit code non-zero and `on_failure` set, dispatch fallback step

### Extension model

Three registries (`StepRegistry`, `TransformRegistry`, `ExtractorRegistry`) each support `register()` for new implementations. Extensions follow the same timeout, budget, and path restrictions as built-ins.

## Design Rules

- **Redirect operators in `cmd` are rejected** — validation in `bashli-core/src/validation.rs` detects `>`, `>>`, `2>`, `2>&1`, `&>`, `|` and returns an error. Use structured `stdout`/`stderr` fields instead.
- **Structural validation lives in bashli-core** (shared by CLI and future MCP mode). Runtime checks (file existence, variable resolution) live in step executors.
- **`--read-only` and `--allowed-paths`** flow from CLI → EngineBuilder → StepContext → write executor. No other layer enforces write guards.
- **`max_output_tokens` vs `TokenBudget`**: when only `max_output_tokens` is set, normalize to TokenBudget with Equal allocation + Truncate overflow. When both set, reject the spec.
- **v1.1+ step types** (If, ForEach, While, Case) must be defined in bashli-core's Step enum from day one (for serde stability), but their executors should return "not yet supported" until implemented.
- Process group kills on timeout — kill the group, not just the direct child.
- Transforms output typed `serde_json::Value`, not only strings.

## v1.0 Scope Boundary

**In scope**: JSON TaskSpec parsing, Sequential/Independent modes, variable capture/interpolation, built-in transforms (including jaq/sed/awk), extraction, LimitSpec, Write/Read/Assert steps, token budget, CLI with compact/pretty JSON output, registry extensibility.

**Deferred to v1.1+**: If/Else, ForEach, While, Case, Parallel modes, retry, YAML input, dry-run, finally blocks, pipe step, function defs, `$import`, MCP mode, DAG execution, caching.

## Implementation Order

Follow this unless a blocker forces adjustment:

1. bashli-core (types + validation)
2. bashli-vars (store + interpolation + escaping)
3. bashli-jq, bashli-sed, bashli-awk (parallel — no internal deps)
4. bashli-runner, bashli-budget (parallel — depend only on core)
5. bashli-transforms (depends on core + jq/sed/awk)
6. bashli-extract (depends on core + jq)
7. bashli-steps (integration layer)
8. bashli-engine
9. bashli-cli

## Third-Party Crate Risks

Verify availability and API surface of `sedregex`, `awk-rs`, and `jaq-*` on crates.io before starting Phase 3A. If `awk-rs` or `sedregex` are unusable, fallback strategies are documented in PLAN.md Risk 6. Pin exact versions.
