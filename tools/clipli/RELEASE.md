# clipli 1.0 Release Notes

`clipli` 1.0 is the stable macOS release line for clipboard inspection, rich-format clipboard I/O, reusable HTML templates, Excel-style table and nested-list generation, privacy-aware clipboard history, and agent-friendly automation.

## Stable In 1.0

- Clipboard inspection, reading, and writing for plain text, HTML, RTF, SVG, PNG, TIFF, and PDF.
- Template capture, paste, render, preview, list/show/edit/delete, lint, search, export/import, and version restore.
- Format conversion between RTF, HTML, plain text, and Jinja2-compatible templates.
- Excel-style table generation from CSV or JSON as editable HTML, SVG, or PNG.
- Nested list generation and path-based editing from JSON, Markdown, HTML, indented lines, or item flags as HTML or Markdown.
- Excel presets: `default`, `finance`, `executive`, `minimal`, and `status`.
- Clipboard history record/list/search/show/restore/prune with source app, type, and date filters.
- Long-running `watch` with deduplication, privacy defaults, and `--max-history` retention.
- Shell completions for bash, zsh, and fish.
- JSON error envelopes with stable top-level `ok`, `error`, and `code` fields.

## Advanced Or Experimental

- `capture --strategy agent` and `--agent-command` are production-usable for controlled agent workflows, but callers should validate agent output and prefer `--dry-run`/`lint` before saving important templates.
- Long-running `watch` is implemented with single-writer history locking and pruning, but operators should still set an explicit retention policy such as `--max-history`.
- MCP server support, a persistent local preview server, image templates beyond Excel-style tables, and non-macOS clipboard backends remain post-1.0 work.

## Installation

From this checkout:

```bash
cd tools/clipli
cargo install --path .
```

From a release archive:

```bash
tar -xzf clipli-1.0.0-aarch64-apple-darwin.tar.gz
install -m 0755 clipli-1.0.0-aarch64-apple-darwin/clipli /usr/local/bin/clipli
```

Verify the binary:

```bash
clipli --version
clipli doctor --json --skip-clipboard
```

Verify a downloaded archive from the directory containing `SHA256SUMS`:

```bash
shasum -a 256 -c SHA256SUMS
```

## Release Packaging

Create a local release archive from the repository root:

```bash
tools/clipli/scripts/package_release.sh
```

The package script builds `clipli`, generates bash/zsh/fish completions, copies release docs, creates a `.tar.gz`, and writes SHA-256 checksum files under `tools/clipli/target/dist/`.

GitHub release automation runs on tags matching `clipli-v*` and attaches the archive plus checksum files.

## Verification

The practical v1.0 gate is:

```bash
cd tools/clipli
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo package --allow-dirty
cargo install --path .
```

Manual GUI checks should be run from a logged-in macOS session when validating a release candidate:

```bash
cargo test -- --ignored
clipli doctor
printf 'Name,Revenue\nAlice,4200\n' | clipli excel --copy-as svg --dry-run > /tmp/clipli-table.svg
clipli list-build --item 'Launch > [x] QA' --copy-as markdown --dry-run
printf '<table><tr><td>preview</td></tr></table>' | clipli preview --output /tmp/clipli-preview.html --json
```

Representative app validation for 1.0 covers Excel/Numbers-compatible table paste, browser/HTML list paste, Markdown list output, RTF/plain-text conversion, SVG/PNG image clipboard generation, and safe history restore dry-runs.
