# bashli Development Plan

This document turns the product and architecture spec in `bashli-spec-final.md` into an execution plan for building bashli from an empty repository to a usable, extensible Rust tool.

It assumes the current state is greenfield: the workspace contains the spec, but the Rust workspace and implementation crates have not been created yet.

## 1. Objectives

Primary objective:

- Deliver `bashli` as a single-binary Rust CLI that accepts a JSON or YAML task specification, executes structured shell workflows, and returns machine-readable JSON results.

Success criteria for the first usable release:

- `bashli` can parse TaskSpec input from inline JSON, file input, and stdin.
- `bashli` can execute sequential and independent multi-step tasks.
- Variables, transforms, extraction, limits, assertions, file operations, token budgeting, and structured results all work end-to-end.
- Redirect syntax inside commands is rejected and replaced by structured JSON fields.
- The crate boundaries and extension registries match the spec so future step, transform, and extractor additions do not require architectural rework.

Secondary objectives:

- Preserve strict crate layering and test isolation.
- Optimize for agent use, not interactive shell ergonomics.
- Keep the implementation aligned with the roadmap so v1.1-v1.4 can land incrementally.

## 2. Delivery Strategy

The spec is broad enough that implementation should not proceed feature-by-feature in random order. The correct sequence is:

1. Establish the workspace skeleton and shared data model.
2. Implement core runtime primitives that all higher layers depend on.
3. Build step executors on top of those primitives.
4. Build the engine orchestration on top of step dispatch.
5. Add the CLI only after the engine contract is stable.
6. Harden with tests, fixtures, validation rules, and performance checks.

This avoids two common failures:

- Building the CLI too early and then rewriting parsing and output logic.
- Building step executors before the variable store, transform registry, and runner contracts are settled.

## 3. Scope by Release

### v1.0 Scope

Deliver all capabilities explicitly called out in the spec's v1.0 roadmap:

- JSON TaskSpec parsing
- Sequential and Independent execution modes
- Variable capture and interpolation
- Built-in transforms, including jaq, sed, and awk-backed transforms
- Built-in extraction methods
- LimitSpec and smart truncation
- Write and Read steps
- Assert steps
- Token budget management
- CLI flags and output modes
- Registry-driven extensibility for steps, transforms, and extractors

Out of scope for v1.0:

- If/Else branching
- ForEach
- Parallel and ParallelN modes
- Retry logic
- YAML input
- Dry-run mode
- Finally block
- Pipe step
- Function definitions and `$import`
- MCP mode
- DAG execution and caching

### v1.1-v1.4 Scope

Implement exactly as staged by the spec roadmap:

- v1.1: control flow and parity gaps
- v1.2: ergonomics and robustness
- v1.3: reuse, composition, extension ecosystem
- v1.4: advanced runtime and scale features

The planning assumption should be that v1.0 is the architecture lock. Later versions should mostly add files and registry entries, not rewrite the core.

## 4. Work Breakdown Structure

### Phase 0: Repository Bootstrap

Goal:

- Create the Cargo workspace, crate layout, CI/test baseline, and project conventions.

Tasks:

- Create the workspace `Cargo.toml` with all planned crates.
- Create `crates/`, `tests/`, `docs/`, and fixture directories per spec.
- Add top-level developer docs:
  - `README.md`
  - `docs/SPEC.md` copied or derived from the final spec
  - `docs/ARCHITECTURE.md`
  - `docs/EXTENDING.md`
- Add baseline toolchain files:
  - `rust-toolchain.toml`
  - `.gitignore`
  - `clippy.toml` if needed
  - `.cargo/config.toml` for workspace defaults
- Establish lint/test commands:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`

Deliverables:

- Compiling empty workspace
- CI-ready directory structure
- Documented development commands

Exit criteria:

- `cargo metadata` succeeds.
- Every declared crate builds with placeholder `lib.rs` or `main.rs` files.

### Phase 1: Core Types and Validation Foundation

Goal:

- Implement `bashli-core` as the stable contract layer for spec input, output, conditions, transforms, extraction metadata, and errors.

Tasks in `bashli-core`:

- Define `TaskSpec`, `GlobalSettings`, execution mode types, I/O routing types, and **all** step variants from the spec's `Step` enum — including `If`, `ForEach`, and any other v1.1+ variants. The types must be complete even if the corresponding executors are not implemented until later. Omitting future variants from the enum now would force a breaking serde change when v1.1 lands.
- Define `TaskResult`, `StepResult`, `TaskError`, and error kind enums.
- Define data-only types for transforms, extraction, limits, conditions, retry, and token budget.
- Implement serde derives and tagged enum representations that match the spec.
- Add a dedicated `validation.rs` module (as named in spec §2.11) containing:
  - redirect detection in `cmd` strings
  - incompatible field combinations
  - invalid capture names (must start with `$`)
  - invalid summary references
  - malformed limits and budget settings
  - rejection of v1.1+ step types with a clear "not yet supported" error until their executors ship
- Clarify the relationship between `GlobalSettings.max_output_tokens` and the `TokenBudget` struct. The spec defines both — `max_output_tokens` is the simple scalar shorthand, while `TokenBudget` adds allocation and overflow strategy. Validation should accept either but normalize them into a single internal representation for `bashli-budget`.

Important design decisions to lock now:

- JSON representation of every enum variant
- distinction between validation errors and execution errors
- how `summary`, verbosity, and captured values appear in output
- whether spec validation lives entirely in `bashli-core` or is split with `bashli-cli`

Recommended outcome:

- Keep structural validation in `bashli-core` so both CLI and future MCP mode can share it.

Exit criteria:

- Round-trip serde tests pass for all major types.
- Validation tests cover redirect rejection and invalid spec combinations.

### Phase 2: Variable Store and Interpolation Engine

Goal:

- Implement `bashli-vars` because nearly every runtime feature depends on variable resolution and escaping.

Tasks:

- Implement `VarStore` with:
  - global variables
  - scope stack support
  - system variable initialization
  - export helpers for summaries
- Implement JSON path traversal for `$VAR.field[2].name`.
- Implement interpolation parser handling:
  - `$VAR`
  - `${VAR}`
  - `$$` (literal dollar escape)
  - `$ENV.NAME` (environment variable access — spec §2.4)
  - nested field and array references (`$VAR.field[2].name`)
- Implement shell escaping for command interpolation.
- Define behavior for unresolved variables by execution mode.

Key edge cases:

- variables containing whitespace or shell metacharacters
- interpolation boundaries like `${VAR}_suffix`
- JSON values interpolated into strings (objects/arrays should serialize to JSON strings, not `[object Object]`)
- accessing missing fields or out-of-range indices
- system variable precedence and immutability
- `$ENV.NAME` when the environment variable is unset

Exit criteria:

- Unit tests prove injection-safe escaping.
- Unit tests cover simple, nested, escaped, and invalid references.

### Phase 3: Transform and Extraction Subsystems

Goal:

- Implement pure, registry-driven data shaping before step execution logic is written.

#### Phase 3A: `bashli-jq`, `bashli-sed`, `bashli-awk`

Tasks:

- Add thin wrappers around `jaq-*`, `sedregex`, and `awk-rs`.
- Hide third-party crate details behind minimal internal APIs.
- Add compilation caching for jaq filters.

Exit criteria:

- Each wrapper crate has focused unit tests and no dependency on internal bashli runtime crates.

#### Phase 3B: `bashli-transforms`

Tasks:

- Implement `TransformFn` and `TransformRegistry`.
- Implement v1.0 transforms in isolated files:
  - trim
  - lines
  - count lines/bytes/words
  - head/tail
  - sort/unique
  - grep
  - json parse and split
  - jq
  - sed
  - awk
  - encoding/hash transforms
  - code block and regex formatting helpers
  - pipe chaining
- Define transform input and output normalization around `serde_json::Value`.

Key decision:

- Normalize transform outputs as typed JSON values, not only strings. This is necessary for capture and downstream field access.

#### Phase 3C: `bashli-extract`

Tasks:

- Implement `ExtractorFn` and `ExtractorRegistry`.
- Implement v1.0 extraction methods:
  - pattern matching
  - line/line range extraction
  - regex extraction with named groups
  - jq-based extraction

Exit criteria for Phase 3:

- Every built-in transform and extractor has deterministic tests.
- Registries can resolve built-in and extension implementations without engine involvement.

### Phase 4: Runner and Budget Runtime Primitives

Goal:

- Implement the two stateful runtime primitives: subprocess execution and output budgeting.

Note: `bashli-runner` depends only on `bashli-core` and `tokio`. `bashli-budget` depends only on `bashli-core`. Neither depends on variables, transforms, or extraction. This means Phase 4 can proceed in parallel with Phase 3 once Phase 1 is complete.

#### Phase 4A: `bashli-runner`

Tasks:

- Implement `CommandRunner` and `RunOpts`.
- Support:
  - cwd overrides
  - environment injection
  - stdin input
  - stdout/stderr capture
  - merge/discard/file routing hooks needed by steps
  - timeouts
  - process group creation and group kill
- Implement signal-friendly shutdown behavior.

Key decisions:

- Use async process execution consistently via `tokio`.
- Keep `bashli-runner` free of variable logic, transforms, and business rules.

#### Phase 4B: `bashli-budget`

Tasks:

- Implement token estimation.
- Implement allocation strategies.
- Implement truncation strategies, especially smart truncation.
- Define how budget charging interacts with per-step limits and summary mode.

Important rule to preserve:

- Per-step limit and global token budget must compose deterministically. A step should first shape its own output, then be charged against the global budget.

Exit criteria:

- Runner integration tests validate spawn, timeout, exit codes, stderr handling, stdin piping, and process-group cleanup.
- Budget tests validate allocation math, truncation behavior, and abort mode.

### Phase 5: Step Executors and Registry

Goal:

- Implement `bashli-steps` as the integration layer that wires variables, transforms, extraction, runner, and budget into step execution.

v1.0 step executors to implement:

- `CmdStep`
- `LetStep`
- `AssertStep`
- `WriteStep`
- `ReadStep`

Tasks:

- Define `StepContext` so executors receive all shared state and services explicitly.
- Implement `StepExecutor` trait.
- Implement `StepRegistry` with built-in and extension resolution.
- Implement command execution flow in `cmd.rs`:
  - interpolate command, cwd, env, and stdin
  - run subprocess
  - apply stdout/stderr routing
  - apply transforms
  - capture transformed result AND run extractions (both consume the same transformed output — extraction does not depend on capture; they are parallel consumers of the same data)
  - enforce limits and budget on the output stored in `StepResult`
  - generate `StepResult`
- Implement the `on_failure` field on `CmdStep`. The spec defines `on_failure: Option<Box<Step>>` — a fallback step to execute when the primary command fails. This requires the step executor to recurse into another step dispatch, so the `StepRegistry` must be accessible from within the cmd executor.
- Implement file steps with path restriction checks and atomic writes. The `--read-only` and `--allowed-paths` CLI flags must be threaded into `StepContext` and enforced here in the write executor, not in the CLI or engine layers.
- Implement assertions against values and literals.

Design risks to resolve early:

- whether `capture` stores raw command output or transformed output
- where stdout and stderr metadata are recorded when summary mode is enabled
- how `on_failure` interacts with sequential mode abort behavior (recommendation: if the fallback itself fails, propagate the error; if it succeeds, mark the original step as "recovered" and continue)

Recommended behavior:

- Capture transformed output by default because the spec describes transforms as applied before capture.
- Both capture and extraction consume the transformed output. Neither depends on the other.
- Record raw execution metadata separately in `StepResult` for debug and full verbosity.

Exit criteria:

- Step executor integration tests pass against a real `StepContext`.
- Extension registration can instantiate a custom step kind without modifying engine code.

### Phase 6: Engine Orchestration

Goal:

- Implement `bashli-engine` as a thin orchestration layer over the step registry.

Tasks:

- Implement `Engine` and `EngineBuilder`.
- Implement v1.0 execution modes:
  - sequential
  - independent
- Process `TaskSpec.let_vars` before the step loop. The spec defines a top-level `let_vars` field that pre-computes variables before any step runs. This must be resolved through the interpolation engine and injected into the `VarStore` before step 0 executes.
- Initialize system variables (`$_CWD`, `$_HOME`, `$_OS`, `$_ARCH`, `$_TIMESTAMP`) at task start.
- Update per-step system variables (`$_STEP_INDEX`, `$_PREV_EXIT`, `$_PREV_STDOUT`) between steps.
- Accumulate per-step results.
- Stop or continue based on execution mode.
- Build final `TaskResult`, `variables`, and `error` fields.
- Implement summary mode and verbosity shaping.

Non-goals for v1.0 engine:

- parallel fan-out
- nested sub-step orchestration
- finally blocks
- DAG scheduling

Exit criteria:

- End-to-end engine tests pass for happy path, failure path, validation failure, and summary mode.

### Phase 7: CLI Surface

Goal:

- Implement `bashli-cli` only after the engine contract is stable.

Tasks:

- Implement clap argument parsing.
- Support input sources:
  - inline JSON spec argument
  - `-f/--file`
  - stdin via `-`
- Add shorthand expansion:
  - bare `cmd`
  - bare step arrays
- Add output formatting:
  - compact JSON
  - pretty JSON
  - debug verbosity support
- Map result state to exit codes:
  - `0` success
  - `1` task failure
  - `2` parse or validation failure

Recommended deferrals:

- Parse YAML in v1.2 as the spec suggests.
- Implement `--schema`, `--dry-run`, and streaming JSONL in v1.2.

Exit criteria:

- CLI E2E tests cover inline JSON, file input, stdin input, and failure exit codes.

### Phase 8: Hardening and Release Readiness

Goal:

- Make v1.0 shippable and keep future versions from forcing architectural rewrites.

Tasks:

- Add fixture-based E2E tests under `tests/fixtures` and golden outputs under `tests/fixtures/expected`.
- Add benchmark scaffolding with `criterion` for the critical paths named in the spec.
- Add release pipeline tasks:
  - reproducible release builds
  - version stamping
  - changelog discipline
  - static binary investigation per target platform
- Audit dependency graph against the spec's layering rules.
- Verify performance targets and capture baseline numbers.

Exit criteria:

- All workspace tests pass.
- Benchmarks exist, even if target numbers are not yet fully met.
- The dependency graph has no internal cycles and matches the documented crate boundaries.

## 5. Milestones

### M0: Workspace Skeleton

- Cargo workspace exists
- All crates compile as placeholders
- CI commands documented

### M1: Core Contracts Locked

- `bashli-core` types stable enough for downstream work
- redirect validation implemented
- serde round-trip tests green

### M2: Runtime Primitives Ready

- variable store complete
- transform and extraction registries operational
- runner and budget primitives tested

### M3: v1.0 Step Layer Complete

- all v1.0 step executors implemented
- command interpolation, capture, transforms, extraction, file I/O, and assertions work together

### M4: v1.0 Engine Complete

- sequential and independent modes work end-to-end
- `TaskResult` shaping stable

### M5: v1.0 CLI Complete

- inline, file, and stdin input supported
- correct exit codes
- compact and pretty JSON output working

### M6: v1.0 Release Candidate

- tests, fixtures, and benchmark scaffolding complete
- documentation aligned with behavior
- no known blockers on core v1.0 scope

## 6. Recommended Implementation Order by Crate

Follow this order unless a concrete blocker forces adjustment:

1. `bashli-core`
2. `bashli-vars`
3. `bashli-jq`, `bashli-sed`, `bashli-awk` (no internal deps — can be developed in parallel with each other)
4. `bashli-runner`, `bashli-budget` (depend only on `bashli-core` — can be developed in parallel with transforms/extraction)
5. `bashli-transforms` (depends on core + jq/sed/awk)
6. `bashli-extract` (depends on core + jq)
7. `bashli-steps` (integration layer — depends on everything above)
8. `bashli-engine`
9. `bashli-cli`

Parallelization opportunities:

- Steps 3 and 4 have no mutual dependencies. `bashli-runner` depends only on `bashli-core` and `tokio`, so it can be built at the same time as the jq/sed/awk wrappers and the transform/extraction registries.
- Steps 5 and 6 can also proceed in parallel since `bashli-extract` does not depend on `bashli-transforms`.

Why this order works:

- It respects the dependency DAG from the spec.
- It maximizes the amount of functionality that can be unit tested before subprocess orchestration is introduced.
- It minimizes interface churn in higher layers.
- It surfaces third-party crate integration issues (jaq, sedregex, awk-rs) early, before higher-layer code depends on them.

## 7. Testing Plan

Testing should be developed alongside each phase, not deferred to the end.

Required test layers:

- Unit tests in `bashli-core` for serde, schema, and validation rules.
- Unit tests in `bashli-vars` for interpolation, escaping, path resolution, and scoping.
- Unit tests in `bashli-jq`, `bashli-sed`, and `bashli-awk` for wrapper behavior and error translation.
- Unit tests in `bashli-transforms` and `bashli-extract` for every built-in variant.
- Integration tests in `bashli-runner` for subprocess and timeout behavior.
- Integration tests in `bashli-steps` for end-to-end step side effects.
- Integration tests in `bashli-engine` for task-mode behavior and result shaping.
- E2E CLI tests using compiled-binary invocation.

Minimum fixture set for v1.0:

- simple single command
- multi-step sequential task
- independent mode with one failure
- capture and interpolation (including `$VAR.field` and `$ENV.NAME` access)
- transform chain (pipe of multiple transforms)
- multi-extract from a single step output
- output limit and smart truncation
- assert pass and fail cases (equals, contains, matches, numeric comparisons)
- assert with `on_fail: fallback` step
- write/read round trip
- write with `create_new` mode when file exists (expect failure)
- redirect validation failure (cmd containing `>`, `2>&1`, etc.)
- token budget truncation with smart strategy
- token budget overflow with abort strategy
- command timeout
- undefined variable in sequential mode (expect abort)
- undefined variable in independent mode (expect warning)
- `on_failure` fallback step on CmdStep
- shorthand input: bare array, bare `cmd` field
- unknown extension step kind (expect exit code 2)
- `let_vars` top-level pre-computation
- summary mode filtering of output variables

## 8. Security and Safety Checklist

These items are mandatory before calling v1.0 complete:

- Shell interpolation always escapes variable values via `bashli-vars/src/escape.rs`.
- Redirect operators in `cmd` are rejected reliably by `bashli-core/src/validation.rs`.
- Write operations respect `create_new`, `read_only`, and allowed-path restrictions. The `--read-only` and `--allowed-paths` CLI flags are threaded from `bashli-cli` into `EngineBuilder`, stored in `StepContext`, and enforced in `bashli-steps/src/write.rs`. No other layer performs write guards.
- Command timeouts kill full process groups, not just direct children.
- Structured errors never leak malformed or partial JSON — even under SIGINT.
- Extension steps are constrained by the same timeout, budget, and path restrictions as built-ins.

## 9. Performance Plan

The spec includes explicit targets. The implementation plan should treat them as measured gates, not aspirations.

Instrumentation tasks:

- Add lightweight timing around parse, validation, interpolation, transform dispatch, step dispatch, and command execution.
- Add `criterion` benchmarks for the functions named in the spec.
- Capture baseline results after M4 and again before M6.

Performance review checkpoints:

- After `bashli-vars` is complete
- After transform registry and jaq cache land
- After engine integration is complete

If targets are missed, optimize in this order:

1. eliminate avoidable allocations in interpolation and transform pipelines
2. cache compiled jaq filters aggressively
3. reduce cloning of `serde_json::Value`
4. trim unnecessary data stored in `StepResult`

## 10. Risks and Mitigations

### Risk 1: Spec breadth delays v1.0

Issue:

- The full document describes both v1.0 and future roadmap features. It is easy to overbuild.

Mitigation:

- Enforce a hard v1.0 feature gate and explicitly defer v1.1+ capabilities.

### Risk 2: Crate boundaries drift from the spec

Issue:

- Convenience shortcuts can collapse layering and make future extension work expensive.

Mitigation:

- Review every new dependency against the documented DAG before merging.

### Risk 3: Ambiguity around transformed versus raw output

Issue:

- Capture, extraction, debugging output, and summary mode can diverge if the data flow is unclear.

Mitigation:

- Lock the command step pipeline early and document it in code comments and tests.

Recommended v1.0 pipeline for CmdStep execution:

1. interpolate `cmd`, `cwd`, `env`, `stdin` fields
2. run command via `bashli-runner`
3. collect raw stdout and stderr byte streams
4. apply I/O routing (merge/discard/file/tee per `stderr`/`stdout` fields)
5. compute displayable stdout content as a string
6. apply transforms (produces the "transformed output")
7. from the transformed output, simultaneously:
   - store into the capture variable (if `capture` is set)
   - run extractions into named sub-variables (if `extract` is set)
8. apply per-step `limit` to the output stored in `StepResult`
9. charge the output against the global token budget
10. generate `StepResult`
11. if exit code is non-zero and `on_failure` is set, dispatch the fallback step

### Risk 4: Async subprocess cleanup is error-prone

Issue:

- Timeouts and signals can leave orphaned child processes.

Mitigation:

- Build process group handling early and test against nested child processes.

### Risk 5: Validation ownership becomes fragmented

Issue:

- If validation is split inconsistently across CLI, engine, and steps, behavior will drift.

Mitigation:

- Keep structural validation centralized in `bashli-core/src/validation.rs`, with runtime checks in step executors only where context is required (e.g., file existence, variable resolution).

### Risk 6: Third-party crate availability and stability

Issue:

- The spec depends on `sedregex` and `awk-rs` as Rust crates for the sed and awk transforms. Neither is a widely-adopted ecosystem crate. They may be unmaintained, have API incompatibilities, or lack features assumed by the spec. The `jaq-*` crates are more established but have undergone breaking API changes between versions.

Mitigation:

- Verify the existence, API surface, and maintenance status of `sedregex`, `awk-rs`, and `jaq-*` crates on crates.io **before** Phase 3A begins. Do not start wrapper crate implementation until the dependency is confirmed usable.
- If `awk-rs` does not exist or is unusable, evaluate alternatives: a minimal hand-rolled field-extraction engine covering the 80% case (`{print $N}` with field separator), or a different crate.
- If `sedregex` is unusable, evaluate implementing the `s/pattern/replacement/flags` syntax directly using the `regex` crate, which is already a dependency.
- Pin exact versions of these crates in `Cargo.toml` to prevent surprise breakage.

### Risk 7: `TokenBudget` and `max_output_tokens` dual specification

Issue:

- The spec defines `GlobalSettings.max_output_tokens` (a simple scalar) and a separate `TokenBudget` struct with allocation strategy and overflow behavior. Both control output size. If the PLAN doesn't resolve how they interact, implementations will diverge.

Mitigation:

- Treat `max_output_tokens` as the shorthand form. When only `max_output_tokens` is set, normalize it to a `TokenBudget` with `Equal` allocation and `Truncate` overflow strategy. When both are set, `TokenBudget` takes precedence and `max_output_tokens` is ignored. Document this in `bashli-core/src/validation.rs` and reject specs that set both.

## 11. Release Plan Beyond v1.0

### v1.1 Plan

Implementation order:

1. extend interpolation for defaults (`${VAR:-fallback}`) and string manipulation (`${VAR//old/new}`, `${VAR^^}`, etc.)
2. add math evaluator in `bashli-vars` (expressions starting with `=` in `let` values)
3. add `IfStep` executor (types already defined in v1.0 `bashli-core`; this adds the executor in `bashli-steps`)
4. add `WhileStep` and `CaseStep` (new types in `bashli-core`, new executors in `bashli-steps`)
5. extend assertions with filesystem predicates (`FileExists`, `DirExists`, `FileContains`, `FileNewer`)
6. add `ForEach` executor and nested-step execution support
7. add parallel and `ParallelN` engine modes
8. add retry logic and backoff

Acceptance criteria:

- all control-flow steps (If, While, Case, ForEach) can execute nested step arrays safely
- variable scoping remains correct across loop and branch boundaries (ForEach creates a scope; If/While/Case do not)
- parallel execution preserves result ordering and respects concurrency bounds

### v1.2 Plan

Implementation order:

1. YAML input
2. `--dry-run`
3. `--schema`
4. streaming JSONL output
5. finally blocks
6. tee support on command steps
7. pipe step
8. diff-aware write mode
9. xargs batching on `ForEach`

Acceptance criteria:

- CLI becomes production-usable for agents and humans running larger tasks
- cleanup and streaming behaviors are reliable under failure scenarios

### v1.3 Plan

Implementation order:

1. `$import`
2. function definitions and call step
3. MCP mode
4. first-party extension crates: HTTP, SQL, Git

Acceptance criteria:

- TaskSpec reuse works without circular imports
- extension author workflow is stable and documented

### v1.4 Plan

Implementation order:

1. caching layer
2. incremental execution
3. watch mode
4. DAG scheduler
5. WASM plugin system
6. metrics export

Acceptance criteria:

- advanced runtime features do not break the simpler v1.x execution model
- dependency-aware scheduling is correct and observable

## 12. Definition of Done

v1.0 is done when all of the following are true:

- All v1.0 scope items in this plan are implemented.
- The crate structure matches the architecture section of the spec.
- Redirect replacement is enforced through validation and structured I/O routing.
- The CLI returns structured JSON and correct exit codes.
- The test matrix described in the spec exists and is passing for v1.0 features.
- Benchmarks are present for the documented critical paths.
- The project is documented well enough for a contributor to add a new transform or step without reverse-engineering the engine.

## 13. Immediate Next Actions

Recommended first implementation sprint:

1. Scaffold the Cargo workspace and all crates.
2. Implement `bashli-core` types and validation.
3. Implement `bashli-vars` completely.
4. Add wrapper crates for jaq, sed, and awk.
5. Add the transform and extraction registries with a small initial built-in set.

If those five items are complete, the project will have locked its core contracts and removed most of the architectural uncertainty.