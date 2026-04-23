# Vizli Render Recipes

Use these patterns to build commands without improvising the payload model.

## Canonical payload reminder

```json
{
  "params": {},
  "datasets": {},
  "meta": {}
}
```

Payload precedence is:

1. sample payload only when explicitly used as the base
2. `--payload`
3. `--dataset`
4. `--param`
5. `--params-file`
6. explicit render flags such as `--theme`, `--width`, `--height`, `--scale`, `--bg`

Later wins.

## Common command shapes

Full payload file:

```bash
vizli render templates/example.vizli.json \
  --payload payload.json \
  --format svg \
  --output out/chart.svg
```

Named datasets plus scalar params:

```bash
vizli render templates/example.vizli.json \
  --dataset table=data.csv \
  --param x_field=date \
  --param y_field=revenue \
  --param title="Revenue over time" \
  --format png \
  --output out/revenue.png
```

Interactive HTML explainer:

```bash
vizli render templates/explainer.vizli.html \
  --param baseline_rate=0.03 \
  --param lift=0.01 \
  --format html \
  --output out/explainer.html
```

Preflight only:

```bash
vizli inspect templates/example.vizli.json
vizli validate templates/example.vizli.json --payload payload.json
vizli render templates/example.vizli.json --payload payload.json --dry-run
```

## Format and engine guardrails

- `svg` engines do not emit HTML
- `html` and `js` usually need browser rendering for PNG or PDF
- Interactive requests usually imply `html`
- Static embeds usually imply `svg` or `png`
- Use `--scale 2` or `--scale 3` when the output is for slides or dense raster export

## Data inspection shortcuts

Use lightweight inspection before mapping params:

```bash
head -n 5 data.csv
jq 'keys' payload.json
jq '.datasets | keys' payload.json
```

If `jq` is unavailable, use the smallest alternative that exposes column or key names.

## Output hygiene

- Set `--output` explicitly when the user gave a file name
- Use `--all` only when the user actually wants every declared format
- Prefer `--theme dark` only when asked or when the template is clearly slide-oriented
