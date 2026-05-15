# Cross-Tool Error Registry

> Status: living registry, v0.1.
> Authoritative platform contract:
> [`AGENT_TOOLS_PLATFORM_SPEC.md`](AGENT_TOOLS_PLATFORM_SPEC.md) (§5).
> Scope: shared error codes, families, and their per-tool aliases. The
> goal is for an agent error handler written once to behave reasonably
> across every `*li` tool.

This registry catalogues:

1. The five **error families** every tool should expose via `category`.
2. The shared **artifact-specific codes** with stable semantics.
3. **Per-tool deviations**: codes a tool emits today that should map
   to a shared code at the next compatible bump.

When a tool needs an error code that is not in the registry, propose a
new code in this file before shipping it. Renames and deprecations are
handled the same way: update this file first.

---

## 1. Error Families

Modeled on `tooli`'s `E1xxx`–`E5xxx` families (see
[`tooli.md`](tooli.md)). Every error must carry a `category` that
maps to one of these families so generic handlers can branch without
parsing the code.

| Family | `category` | Meaning | Default exit code |
|---|---|---|---:|
| `E1xxx` | `input` | Bad flags, malformed JSON, schema violation, ambiguous selector that the *caller* can disambiguate. | 2 |
| `E2xxx` | `auth` | Missing token, denied scope, expired credential. | 30 |
| `E3xxx` | `state` | Not found, conflict, stale preimage, ambiguous selector against *artifact* state. | 10 |
| `E4xxx` | `runtime` | Network failure, missing binary, target app/bridge unavailable, rate limit. | 70 |
| `E5xxx` | `internal` | Tool bug, unreachable branch, panic-recovered failure. | 70 |

Per-tool exit-code conventions (e.g. `jirali`'s 0–8 codes; `mdli`'s
0–4 + 64) override the defaults above. Always check the tool's README
for its exit-code table.

---

## 2. Shared Artifact Codes

These codes have the same meaning everywhere they appear. New tools
should adopt them by name; old tools should map their existing codes
to these by the next compatible bump.

### 2.1 Selector / addressing (family `E1xxx` or `E3xxx`)

| Code | Family | Meaning | `is_retryable` | First/canonical use |
|---|---|---|---|---|
| `E_AMBIGUOUS_SELECTOR` | `input` | Selector matched more than one structure. `details.matches` lists candidates so an agent can disambiguate. | true (with a refined selector) | `mdli` |
| `E_SELECTOR_NOT_FOUND` | `state` | Selector matched zero structures. | true (with a different selector) | `mdli` |
| `E_DUPLICATE_ID` | `state` | Stable ID present more than once in the artifact. | false (artifact must be repaired first) | `mdli` |
| `E_INVALID_SELECTOR` | `input` | Selector is syntactically invalid (e.g. malformed A1, malformed UUID). | true (after correction) | Recommended. |

### 2.2 Mutation safety (family `E3xxx`)

| Code | Meaning | `is_retryable` | First/canonical use |
|---|---|---|---|
| `E_STALE_PREIMAGE` | Input bytes changed since the plan was computed; refuse to write. | true (re-inspect, re-plan) | `mdli` |
| `E_BLOCK_LOCKED` | Generated region is marked locked; edit refused unless `--force-locked`. | false (operator decision) | `mdli` |
| `E_BLOCK_MODIFIED` | Generated region was edited outside the tool; managed checksum no longer matches. | false (resolve via three-way conflict) | `mdli` |
| `E_CONFLICT` | Generic concurrent-edit conflict; sidecar may have been written. | true (after manual reconciliation) | Recommended. |

### 2.3 Validation (family `E1xxx`)

| Code | Meaning | First/canonical use |
|---|---|---|
| `E_VALIDATION_MISSING_SECTION` | Required section absent from the artifact. | `mdli` |
| `E_VALIDATION_MISSING_TABLE` | Required named table absent. | `mdli` |
| `E_VALIDATION_TABLE_COLUMNS` | Table columns differ from schema. | `mdli` |
| `E_VALIDATION_TABLE_KEY` | Table key differs from schema. | `mdli` |
| `E_VALIDATION_MISSING_BLOCK` | Required managed block absent. | `mdli` |
| `E_VALIDATION_BLOCK_LOCK` | Managed block lock state differs from schema. | `mdli` |
| `E_VALIDATION_SCHEMA_INVALID` | Validation schema itself is missing or wrong version. | `mdli` |
| `E_SCHEMA_MISMATCH` | Recipe / job / plan version not understood by this tool version. | `mdli` |

### 2.4 Auth and external dependencies (family `E2xxx` / `E4xxx`)

| Code | Family | Meaning | `is_retryable` | First/canonical use |
|---|---|---|---|---|
| `E_AUTH_MISSING` | `auth` | No credentials configured. | false (operator must configure) | `framerli`, `notionli` |
| `E_AUTH_INVALID` | `auth` | Credentials present but rejected by upstream. | false (operator must rotate) | Recommended. |
| `E_AUTH_SCOPE` | `auth` | Token valid but lacks the required scope/permission. | false | Recommended. |
| `E_RATE_LIMITED` | `runtime` | Upstream rate limit hit; `details.retry_after_ms` set when known. | true (after backoff) | Recommended. |
| `E_BRIDGE_DISCONNECTED` | `runtime` | Live-app bridge not connected (e.g. PowerPoint add-in offline, Node sidecar down). | true (after the operator brings the bridge up) | Recommended. Today: `deckli` `no_addin`, `framerli` partial. |
| `E_NOT_IMPLEMENTED` | `runtime` | Command surface exists but backend/bridge for it is not yet wired. | false | `framerli` |
| `E_TIMEOUT` | `runtime` | Operation exceeded `--timeout`. | true | Recommended. |

### 2.5 Artifact fidelity (family `E4xxx`)

| Code | Meaning | `is_retryable` | First/canonical use |
|---|---|---|---|
| `E_PARTIAL_FIDELITY` | Operation succeeded but some artifact features may not have round-tripped (e.g. `xli`'s `umya-spreadsheet` fallback path on chart-bearing workbooks). Surfaced today through the xli envelope `warnings` field; promotion to a structured code is recommended. | n/a (operation already succeeded) | Recommended; today emitted as warnings by `xli`. |
| `E_INVALID_UTF8` | Input bytes are not valid UTF-8. | true (after re-encoding) | `mdli` |

---

## 3. Per-Tool Code Maps

The current state of error codes per tool, with mapping to the
families above. This is the input to harmonization, not the target.

### `mdli`

Stable string codes prefixed `E_`. Already aligned with §2 for
selector, mutation safety, and validation codes. Maps cleanly to
families when `category` is added.

### `xli`

Errors documented per command in `tools/xli/README.md`. Partial-fidelity
warnings on the `umya-spreadsheet` path are emitted in the envelope
`warnings` field today; promotion to `E_PARTIAL_FIDELITY` is recommended
at the next compatible bump.

### `vaultli`

Validation findings (broken sources, duplicate IDs, dangling refs,
stale index state) surface through `validate`. Recommended: standardize
on `E_VALIDATION_*` codes for parity with `mdli`.

### `clipli`

Today emits a flattened `{ok, error, code}` form for failure. Codes
are tool-specific strings (not yet mapped). Recommendation: graduate
to the structured `error` object and adopt shared codes for
clipboard-state mismatches.

### `jirali`

Exit codes 0–8 are Jira-shaped (0 success, 1 general, 2 usage,
3 not-found, 4 permission, 5 conflict, 6 rate-limited, 7 validation,
8 timeout). Recommendation: surface a `category` in the JSON envelope
that maps each exit code to a family (e.g. exit 4 → `auth`, exit 6 →
`runtime`).

### `notionli`

Exit codes per the PRD; `E_AUTH_MISSING` is shared with `framerli`.
Recommendation: extend coverage with `E_RATE_LIMITED` and
`E_AUTH_SCOPE` as those scenarios appear in real workflows.

### `framerli`

Stable `E_*` codes including `E_AUTH_MISSING` and `E_NOT_IMPLEMENTED`.
Error envelope uses `hint` and `retryable` (vs. the platform's
`suggestion` and `is_retryable`); see the tool's "Platform
Conformance" section for the planned alignment.

### `deckli` (spec)

Currently uses lowercase string codes (`unknown_command`,
`shape_not_found`, `no_addin`). Recommendation: graduate to the
shared `E_*` codes — `no_addin` becomes `E_BRIDGE_DISCONNECTED`,
`shape_not_found` becomes `E_SELECTOR_NOT_FOUND`, etc., before v1.

### `docli` (spec)

Uses upper-snake codes without an `E_` prefix (e.g. `INVALID_TARGET`).
Recommendation: add the `E_` prefix and a `category` field at v1; map
existing names onto the shared registry where the semantics align
(`INVALID_TARGET` → `E_SELECTOR_NOT_FOUND` in `state` family).

### `bashli` (spec)

Greenfield; should adopt the shared codes from day one. Likely
candidates: `E_TIMEOUT`, `E_RATE_LIMITED` (where applicable for HTTP
steps), and a new `E_STEP_FAILED` (family `state`) for non-zero exit
codes inside the pipeline.

### `vizli`

Should expose `E_TEMPLATE_PARSE`, `E_TEMPLATE_MISSING_DATASET` (already
in `mdli`'s registry), and a new `E_RENDER_FAILED` (family `runtime`)
for engine-level failures. Specs at `tools/vizli/PLAN.md` define
strict-undefined behavior and offline guarantees that map cleanly to
existing codes once added.

---

## 4. Adding a New Code

When proposing a new shared code:

1. **Confirm no existing code fits.** Check §2; check the per-tool
   maps in §3.
2. **Pick a family.** Use the table in §1; the `category` value
   should be one of `input`, `auth`, `state`, `runtime`, `internal`.
3. **Decide retryability.** Is this recoverable by the agent without
   operator action?
4. **Choose a stable name.** `E_<NOUN>_<CONDITION>`. Avoid tool-specific
   names; prefer artifact-level concepts.
5. **Add it to §2 with semantics, retryability, and first use.**
6. **Open a tracking note** in `tooli_feedback.md` if the new code
   reveals a gap in the platform spec.

Renames go through the same process; old codes stay in the registry
under a "deprecated" subsection (to be added the first time it is
needed) until every tool has migrated.

---

## 5. Recovery Suggestion Conventions

Each error's `suggestion` object should follow this shape (spec §4.5):

```json
{
  "action": "retry_with_modified_input",
  "fix": "Run `mdli id list report.md` to see available IDs.",
  "example": "mdli section get report.md --id cashplus.okr"
}
```

Recommended `action` values for shared codes:

| Code | Typical `action` |
|---|---|
| `E_AMBIGUOUS_SELECTOR` | `disambiguate_selector` |
| `E_SELECTOR_NOT_FOUND` | `retry_with_modified_input` |
| `E_STALE_PREIMAGE` | `reinspect_and_replan` |
| `E_BLOCK_MODIFIED` | `resolve_conflict` |
| `E_BLOCK_LOCKED` | `unlock_or_force` |
| `E_AUTH_MISSING` | `configure_credentials` |
| `E_AUTH_INVALID` | `rotate_credentials` |
| `E_AUTH_SCOPE` | `request_additional_scope` |
| `E_RATE_LIMITED` | `backoff_and_retry` |
| `E_BRIDGE_DISCONNECTED` | `start_bridge` |
| `E_NOT_IMPLEMENTED` | `use_alternative_command` |
| `E_TIMEOUT` | `increase_timeout_or_split` |

These are guidance, not enforcement. A tool may emit a more specific
`action` if it serves the agent better.
