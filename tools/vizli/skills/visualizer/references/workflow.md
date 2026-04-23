# Vizli Operator Workflow

This is the compact checklist for running vizli safely.

## 1. Pin down the request

Before selecting a template, extract:

- Output form: chart, diagram, explainer, report visual
- Output format: `svg`, `png`, `html`, or `pdf`
- Output destination: explicit file path if the user gave one
- Data sources: CSV, JSON, payload file, or inline values
- Required constraints: dark theme, static output, interactive output, slide-ready density

If one of these is missing, make the smallest safe assumption and state it.

## 2. Search templates through sidecars

Search `*.md` files that sit next to `.vizli.*` templates. Prefer:

1. Tag match
2. Description match
3. Title match
4. Directory browse if the request is broad

Read only a few promising sidecars before deciding.

## 3. Resolve the payload

Map the user's inputs onto the canonical payload:

```json
{
  "params": {},
  "datasets": {},
  "meta": {}
}
```

- Put scalar configuration in `params`
- Put named files or tables in `datasets`
- Put format, width, height, theme, and scale in `meta` via CLI flags

Never guess required field names. Confirm them from the sidecar or schema.

## 4. Preflight when needed

Use these checks before a risky render:

- `vizli inspect <template>` when you need the schema summary or output list
- `vizli validate <template> --payload ...` when payload fit is uncertain
- `vizli doctor` when the failure looks environment-related

## 5. Render explicitly

Prefer commands that are explicit about:

- Template path
- Payload file or dataset flags
- Param overrides
- Output format
- Output file
- Theme or scale if requested

Use `--dry-run` first when you are still testing payload resolution.

## 6. Verify and deliver

After rendering:

- Confirm the output file exists
- Confirm the extension and requested format match
- Mention the template used and the key params or datasets
- If output looks wrong, report the mismatch instead of pretending success

## Stop conditions

Stop and explain the blocker when:

- No sidecar-backed template actually fits the request
- Required params cannot be resolved from the available data
- The engine cannot emit the requested format
- The environment lacks required runtime dependencies
