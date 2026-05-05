# docli Development Plan

> Status: living plan, v0.1 (initial draft, 2026-05-05).
> Authoritative spec: [`docli-spec.md`](./docli-spec.md).
> Platform contract: [`../../docs/AGENT_TOOLS_PLATFORM_SPEC.md`](../../docs/AGENT_TOOLS_PLATFORM_SPEC.md).
> Cross-tool error registry: [`../../docs/error-registry.md`](../../docs/error-registry.md).

This file plans the next phase of `docli` work. It exists because the
spec is broad (2,169 lines) and the implementation is *much* further
along than the platform-spec maturity placement suggested. Without a
plan we will either re-implement what already exists or chase scope
that the spec only sketches.

The plan is opinionated, sequenced, and intended to be redirected by
the maintainer rather than executed blindly.

---

## 1. Current State (audited 2026-05-05)

### 1.1 What compiles and what passes

- The Rust workspace builds cleanly. Members:
  `docli-cli`, `docli-core`, `docli-query`, `docli-patch`,
  `docli-create`, `docli-schema`, `docli-render`, `docli-kb`.
- `cargo test --workspace` reports **161 tests passing, 0 failing**
  across:
  - `docli-cli` integration tests: `inspect`, `validate`,
    `edit_roundtrip`, `review_roundtrip`.
  - `docli-patch` unit tests: 84 (the heaviest crate).
  - `docli-core`, `docli-query`, `docli-schema`, `docli-create`,
    `docli-kb`: smaller unit-test pockets each.

### 1.2 CLI surface today

`docli --help` exposes 17 subcommands:

```
inspect   validate   ooxml   kb        schema   doctor
read      create     template edit     run      review
finalize  diff       convert  extract  merge
```

End-to-end probes against `tests/fixtures/minimal.docx` confirm at
least these emit a stable JSON envelope:

| Command | Verified output |
|---|---|
| `inspect` | Full structure: `source_hash`, `entry_count`, `paragraphs`, `headings`, `tables`, `images`, `bookmarks`, `comments.count`, `tracked_changes.{count, insertions, deletions, authors}`. |
| `validate` | Structural issues with stable codes (e.g. `missing-required-part`), `error_count`/`warning_count`, `repaired` flag. |
| `read` | Paragraph extraction with style + text. |
| `doctor` | Probes `pandoc`, `soffice`, `pdftoppm`; reports KB root + temp writability. |
| `schema` | Emits JSON Schemas for `Job`, `Operation`, `Target`, `ContentBlock`, `StyleOverride`, etc. — already very rich. |

Edit/review/finalize have integration tests proving end-to-end
round-trip behavior; the exact envelope shape and edge cases are not
yet audited.

### 1.3 Envelope and error shape today

```json
{
  "ok": true,
  "command": "inspect",
  "data": { ... },
  "warnings": [],
  "elapsed_ms": 0
}
```

Errors: `{ ok: false, command, error: { code, message, ... } }`.
Codes are upper-snake without an `E_` prefix
(e.g. `INVALID_TARGET`).

This matches the deviation already documented in
[`docli-spec.md` § Platform Conformance](./docli-spec.md).

### 1.4 Maturity placement

Best-effort against the platform spec ladder (§15):

> **Level 4–5** (atomic mutation in flight; no SKILL.md or
> cross-tool integration yet).

The platform spec / roadmap currently classify `docli` at level 0–1.
**That is wrong** and will be corrected in `docs/tool-roadmap.md`
once this plan is committed and Track A confirms the parity matrix.

### 1.5 Repo cruft to clear

These are almost certainly stray Finder copies and need explicit
maintainer approval before deletion:

- `tools/docli/docli/` — a full nested duplicate of the workspace
  (Cargo.toml + every crate).
- `tools/docli/docli-companion/{pyproject 2.toml, pyproject 3.toml}`.
- `tools/docli/docli-companion/src/docli_companion 2/`.

Tracked in Track F below.

---

## 2. Goals

`docli` should reach **platform maturity level 7** — fixture-backed
contract tests, cross-tool integration tested, SKILL.md, full
mutation-safety contract — without breaking the integration tests
already in place.

Concretely, by the end of this plan an agent should be able to:

1. `docli inspect` an arbitrary `.docx` and trust the structural map.
2. Run a sequence of narrow edit verbs in a dry-run plan, review the
   structured plan, and apply atomically with preimage protection.
3. Compose a review workflow: comment, track-replace, accept/reject,
   strip — with stable error codes for every refusal mode.
4. Generate a greenfield `.docx` from a YAML spec sourced from a
   `kb://` template.
5. Validate the output against hard invariants and KB rules.
6. Diff the result against a previous version semantically.
7. Compose into the cross-tool report workflow:
   `vaultli search → mdli/docli generate → xli/vizli assets → validate`.

---

## 3. Out of Scope (explicitly)

- Re-implementing what the workspace already does. Track A audits
  and documents reality before we touch code.
- Replacing `docx-rs` for the create backend. The spec keeps it
  behind a `CreateBackend` trait; we honor that.
- Cross-platform install, Homebrew formula, signed binaries.
  Defer until the contract is firm.
- An MCP shim. The CLI contract is canonical (platform spec §11);
  MCP is a later projection.
- The Python companion (`docli-companion`). The Rust core is the
  primary surface; companion work happens after the core is stable.

---

## 4. Tracks

Six tracks, A–F. Each is independently committable. Recommended
sequence: **A → B → C → D → E**, with **F** any time the maintainer
gives the go-ahead.

### Track A — Audit & Harden (1–2 sessions)

**Goal.** Eliminate "we don't know what works" as a blocker.

**Deliverables.**

1. `tools/docli/PARITY_MATRIX.md` — every command from the spec on
   one axis; current behavior, integration-test coverage, gaps,
   known bugs on the other.
2. End-to-end probes for every subcommand against the existing
   `tests/fixtures/minimal.docx` and at least one richer fixture
   (heading-bearing, tracked-change-bearing, table-bearing,
   comment-bearing, image-bearing — possibly via a small fixture
   builder).
3. Bug fixes for any obvious failures surfaced by the probes
   (separate commits per bug).
4. A new `tests/fixtures/` row of golden envelope outputs so
   shape regressions become test failures.

**Acceptance.**

- Every subcommand has at least one verified happy-path probe.
- The parity matrix has zero "?" rows.
- `cargo test --workspace` is still green.

### Track B — Platform Conformance (1 session)

**Goal.** Move `docli`'s envelope and error shape onto the platform
contract while pre-v1 churn is still cheap. Done before downstream
skills can ossify.

**Deliverables.**

1. **Envelope rename.** `data` → `result`; fold `elapsed_ms` and
   any other top-level metadata into a `meta` block:

   ```json
   {
     "ok": true,
     "command": "inspect",
     "result": { /* same as today's data */ },
     "meta": {
       "tool": "docli.inspect",
       "version": "0.1.0",
       "duration_ms": 0,
       "dry_run": false,
       "warnings": []
     }
   }
   ```

   Keep `command` at top level (useful routing context).

2. **Error codes.** Add `E_` prefix and a `category` field
   (`input` / `auth` / `state` / `runtime` / `internal`). Map each
   existing code:

   | Today | After |
   |---|---|
   | `INVALID_TARGET` | `E_SELECTOR_NOT_FOUND` (state) |
   | `MISSING_FILE` (or equivalent) | `E_INPUT_FILE_MISSING` (input) |
   | (add full list in Track A's parity matrix) | … |

3. **`docs/error-registry.md` updates.** Add the per-tool map for
   `docli` once the rename lands.

4. **Suggestion object.** Replace any string `suggestion` with the
   structured form `{action, fix, example}` (platform spec §4.5).

5. **Global flags.** Add (or align): `--dry-run`, `--quiet`,
   `--verbose`, `--no-color`, `--timeout`, `--idempotency-key`,
   `--help-agent`, `--agent-manifest`. Many already exist via
   `clap`'s defaults; this is gap-fill, not a redesign.

6. **`--agent-manifest`.** Emit a single JSON document containing
   every command, its capabilities (`readOnlyHint`,
   `idempotentHint`, `destructiveHint`, `openWorldHint`), error
   codes, and examples. The shape is the platform's discovery
   surface (spec §11).

7. **Update `docli-spec.md` Platform Conformance** to remove
   resolved deviations and note remaining ones.

**Acceptance.**

- Every JSON output uses the new envelope.
- Every error has `code`, `category`, `message`, `details`,
  `suggestion`, `is_retryable`.
- `docli schema --result <Name>` works for any result type
  (parity with `xli`).
- Integration tests updated and passing.

### Track C — Mutation Safety Completion (2–3 sessions)

**Goal.** Land the spec's centerpiece: shadow-package atomic
commits with explicit durability, preimage protection, structured
edit plans, and three-way conflict resolution.

This is `docli`'s biggest differentiator and the platform's
strongest example of the §6 mutation safety model.

**Deliverables.**

1. **`--durability {fast, durable, paranoid}`** wired through every
   mutating command (`edit`, `run`, `review`, `finalize`,
   `convert`, `merge`, `create`, `template render --write`).
   Default: `durable`. See spec §2 for semantics.

2. **Preimage hash refusal.** `--preimage-hash <sha256>` on every
   mutating command. Mismatch → `E_STALE_PREIMAGE` with
   `details.expected` and `details.actual`. Make `inspect.result.source_hash`
   the canonical preimage source.

3. **Structured edit plans.** `--dry-run` (or default-to-plan,
   per spec §6.2) emits a `plan` block with:
   - per-op address (resolved selector)
   - per-op type (`edit.replace`, `review.comment`, …)
   - per-op preimage / postimage (where applicable)
   - aggregate `preimage_hash` of the source archive

4. **Atomic commit pipeline.** Verify the 12-step shadow-package
   pipeline in `docli-core/src/pipeline.rs` matches spec §2:
   shadow path on the same filesystem, fsync temp, fsync parent,
   reopen + revalidate (paranoid mode), commit journal entry.

5. **Three-way conflict sidecar.** When `--on-modified three-way`
   is requested and the artifact has changed since the recorded
   preimage, write `<file>.docli.conflict` containing recorded /
   on-disk / incoming bytes (model: `mdli`'s
   `<file>.mdli.conflict`).

6. **Audit log.** Append every mutation to
   `~/.local/share/docli/audit.ndjson` (model: `framerli`'s
   `state/audit.ndjson`). Disable with `--no-audit`.

7. **Hard invariants enforcement.** Confirm the
   `docli-schema/src/invariants.rs` rules from spec §2 are wired
   into the commit pipeline (not just unit-tested in isolation):
   - unified `w:id` allocator
   - tracked-change structural validity
   - comment range sibling enforcement
   - relationship + content-type consistency
   - required package parts
   - table structural integrity
   - `xml:space="preserve"` auto-add
   - `durableId`/`paraId` range repair
   - reopen-after-write validation

**Acceptance.**

- Every mutating command refuses on stale preimage.
- A new `tests/mutation_safety.rs` integration suite covers:
  - happy-path apply with preimage match
  - refusal on preimage mismatch
  - durability mode selection
  - three-way conflict sidecar emission
  - audit log entry shape
  - reopen-and-validate failure path
- `cargo test --workspace` green.

### Track D — Greenfield Content Path (2–3 sessions)

**Goal.** `docli create` from a YAML spec, with `kb://` template
resolution, becomes the killer demo for cross-tool composition.

**Deliverables.**

1. **`docli create`** — verify and complete the YAML-spec → DOCX
   path through the `docx-rs` `CreateBackend`. The schema is
   already rich (see `Job`/`ContentBlock` JSON schemas):
   `heading1/2/3`, `paragraph`, `bullets`, `numbers`, `table`,
   `image`, `page_break`, `toc`, `columns`. Fill any gaps the
   parity matrix surfaces.

2. **`kb://` resolver.** Wire `docli-kb/src/resolver.rs` so a
   spec can reference `kb://templates/quarterly_review.yaml`,
   resolved against `--kb-path` or `DOCLI_KB_PATH`.

3. **`template render`** — minijinja over a YAML spec template,
   producing the resolved spec, then handing to `create`. The
   `--write` path goes through the same atomic commit pipeline
   as Track C.

4. **One real fixture template** under
   `tools/docli/fixtures/templates/quarterly_review.yaml.j2` with
   a sample data file, exercised by an integration test that
   produces a byte-stable output (golden test).

5. **A small companion vault** under
   `tools/docli/fixtures/kb/` so `docli kb list` and
   `docli kb get` have something to discover.

**Acceptance.**

- `docli create --spec brief.yaml --out /tmp/out.docx` produces
  a Word-compatible file (verified by `docli inspect /tmp/out.docx`).
- `docli template render --template kb://templates/quarterly_review --data data.json --out /tmp/out.docx` works end-to-end.
- Golden test guards round-trip stability.

### Track E — SKILL.md and Cross-Tool Loop (1 session)

**Goal.** Make `docli` composable inside agent workflows. Drag it
to platform maturity level 6+.

**Deliverables.**

1. `tools/docli/SKILL.md` modeled on `mdli`'s — frontmatter
   triggers, when-to-use / when-not, agent contract, exit codes,
   error codes (deduped against the registry), recommended
   workflows:
   - "inspect → narrow edit → review → finalize" loop
   - "review board" loop (track-replace + comment, hand to human,
     accept/reject)
   - "greenfield from KB template" loop
   - failure-recovery patterns (`E_STALE_PREIMAGE`,
     `E_BLOCK_LOCKED`, `E_VALIDATION_*`, `E_AUTH_MISSING` for KB
     access, etc.)

2. A composition recipe demonstrating
   `vaultli search → mdli context → docli create/edit → docli validate`
   end-to-end. Lives under `tools/docli/RECIPES.md` or as a fixture.

3. `docs/skills.md` updated to list the new skill.

4. `docs/tool-roadmap.md` updated with `docli` at the corrected
   maturity level.

5. Update the platform spec's tool-family taxonomy table with the
   corrected maturity number.

**Acceptance.**

- `docli` appears in the "Active" skills inventory.
- The cross-tool recipe runs end-to-end on a clean checkout.
- An external agent reading just the SKILL.md can complete one
  full loop without falling back to `--help`.

### Track F — Cruft Cleanup (5 minutes once approved)

**Awaits explicit maintainer approval** because it deletes files.

**Deliverables.**

1. `git rm -r tools/docli/docli/` (the nested duplicate
   workspace).
2. `git rm tools/docli/docli-companion/pyproject\ 2.toml tools/docli/docli-companion/pyproject\ 3.toml`.
3. `git rm -r 'tools/docli/docli-companion/src/docli_companion 2/'`.

Done in a single commit, after confirming `git diff` shows no
real content lost.

---

## 5. Sequencing & Risk

| Step | Track | Why this order | Risk |
|---|---|---|---|
| 1 | A | We're flying blind on what works. Everything downstream depends on it. | Low. Pure investigation + small fixes. |
| 2 | B | Cheapest moment to migrate the envelope is now. Doing C on the old envelope means doing it twice. | Low–Medium. Touches every command's output but is mechanical. Integration tests will catch regressions. |
| 3 | C | The biggest user-visible differentiator and the platform's reference mutation-safety implementation. Builds on B's preimage-hash plumbing. | Medium. OOXML edge cases will show up. Mitigated by 84 existing `docli-patch` tests. |
| 4 | D | Greenfield is the most agent-visible win. Cheap once C is firm. | Low–Medium. `docx-rs` has known sharp edges; the trait hides them. |
| 5 | E | Polish and composition. Worth doing only after the core is stable. | Low. |
| ⟂ | F | Independent. Run any time. | None once approved. |

If schedule pressure forces a cut, **A and B alone** still ship
real value: the tool will be honestly classified, behaviorally
documented, and platform-conforming. C is the next-most-valuable
cut. D and E can wait a release.

---

## 6. Acceptance for the Whole Plan

`docli` is "comprehensively developed" when:

- [ ] Parity matrix exists and has zero unknown rows.
- [ ] Envelope and error shape match the platform contract.
- [ ] Mutation safety: `--durability`, preimage hash, structured
      plan, conflict sidecar, audit log all wired and tested.
- [ ] Greenfield create works from a `kb://` template.
- [ ] SKILL.md exists and an agent can complete a full loop from it.
- [ ] One cross-tool recipe (vaultli + mdli + docli) runs end-to-end.
- [ ] `docs/tool-roadmap.md` and the platform spec list `docli` at
      level 7.
- [ ] `cargo test --workspace` is green throughout, with
      additional integration suites covering each new behavior.

---

## 7. What This Plan Is Not

- A spec replacement. `docli-spec.md` is still the design
  document. This plan is the work breakdown.
- A schedule. Sessions are rough effort estimates, not calendar
  commitments.
- An invitation to skip the audit. Track A is non-negotiable.
- A claim that the spec's full 17-command surface will graduate
  to level 8 in this round. Several spec-only commands
  (`merge`, `extract`, advanced `convert`) may stay at MVP
  beyond this plan.

---

## 8. Open Questions

These are decisions to surface before implementation, not after.

1. **Default-to-plan vs. explicit `--write`?** Spec §6.2 of the
   platform contract recommends "default to plan" for high-risk
   artifacts. `mdli` and `notionli` follow that. `xli` uses
   `--dry-run`. Pick one for `docli`. Recommendation:
   default-to-plan, matching the artifact-modification cohort.
2. **Where does the audit log live?** `framerli` puts it under
   the config home; `notionli` uses the data home. Recommendation:
   `~/.local/share/docli/audit.ndjson` (data home), with
   `--audit-path` override.
3. **Should `docli` adopt `kb://` URIs as the platform-wide vault
   addressing scheme?** The platform spec lists this as an open
   question (§16, item 9). Doing it here sets a precedent —
   coordinate with `vaultli`'s authoritative scheme before
   committing.
4. **Numeric vs. string error families.** This plan uses string
   `E_*` codes (matching `mdli`/`framerli`/`notionli`) and adds
   `category`. The platform spec leaves both forms acceptable.
   Confirm that `docli` does **not** need numeric `E1xxx` codes.
5. **Companion crate.** `docli-companion` (Python) is in tree but
   out of scope here. Confirm it stays parked until the Rust
   core stabilizes.

---

## 9. Pointers

- Platform contract: [`../../docs/AGENT_TOOLS_PLATFORM_SPEC.md`](../../docs/AGENT_TOOLS_PLATFORM_SPEC.md)
- Error registry: [`../../docs/error-registry.md`](../../docs/error-registry.md)
- Skills guide: [`../../docs/skills.md`](../../docs/skills.md)
- Reference tool for the mutation safety model: [`../mdli/README.md`](../mdli/README.md)
- Reference tool for atomic + fingerprint workbook mutation: [`../xli/README.md`](../xli/README.md)
- This tool's spec: [`./docli-spec.md`](./docli-spec.md)
- This tool's Platform Conformance section: see the bottom of `docli-spec.md`.
