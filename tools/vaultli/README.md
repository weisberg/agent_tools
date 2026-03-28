# vaultli

`vaultli` now has two explicit implementation homes:

- `py/` — the current Python implementation, which remains the feature-complete reference version
- `rs/` — the new Rust port in progress

The v1.0 source of truth remains `vaultli-spec-v1.0.md`.

## Python implementation

Run from the repo root with:

```bash
python -m tools.vaultli ...
```

The Python implementation lives in `tools/vaultli/py/` and still provides the full current command surface.

## Rust implementation

The Rust port lives in `tools/vaultli/rs/`.

Current implemented slice:

- `root`
- `init`
- `make-id`
- `infer`
- `index`
- `search`
- `show`

This is intentionally a preview port. Python remains the reference implementation while Rust functionality is built out.

## Validation

```bash
uv run pytest tests/test_vaultli.py
cd tools/vaultli/rs && cargo test && cargo build
```
