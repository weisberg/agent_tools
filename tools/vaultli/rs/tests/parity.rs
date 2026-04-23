//! Behavioral parity tests against the Python reference implementation.
//!
//! These tests only run if the `python3` interpreter can import `vaultli`.
//! Set `VAULTLI_PY_PATH` to the directory that should be placed on
//! `PYTHONPATH` (typically the parent of the `vaultli` Python package). If the
//! env var is unset, this file falls back to the in-repo layout
//! (`<crate>/../py`, importable as `vaultli`). If neither works, the tests
//! are skipped so `cargo test` stays green in minimal environments.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

const VAULT_MARKER: &str = ".kbroot";

fn python_path_dir() -> PathBuf {
    if let Ok(value) = std::env::var("VAULTLI_PY_PATH") {
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    // In-repo fallback: `<crate>/../` contains the `py/` package that we can
    // alias to `vaultli` via a PYTHONPATH that points at the vaultli folder.
    // The repo ships `__init__.py` at `tools/vaultli/` which re-exports `py`,
    // so placing `tools/` on PYTHONPATH makes `import vaultli` work.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn python_available() -> bool {
    let status = Command::new("python3")
        .arg("-c")
        .arg("import vaultli")
        .env("PYTHONPATH", python_path_dir())
        .status();
    matches!(status, Ok(s) if s.success())
}

fn run_python(code: &str) -> Option<Value> {
    let output = Command::new("python3")
        .arg("-c")
        .arg(code)
        .env("PYTHONPATH", python_path_dir())
        .output()
        .ok()?;
    if !output.status.success() {
        eprintln!(
            "python stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    serde_json::from_str(&stdout).ok()
}

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("vaultli-parity-{name}-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn infer_matches_python_for_common_inputs() {
    if !python_available() {
        eprintln!("python vaultli not importable; skipping parity test");
        return;
    }

    let root = temp_dir("infer");
    fs::write(root.join(VAULT_MARKER), "").unwrap();
    let cases = [
        ("docs/user-guide.md", "# Guide\n"),
        ("queries/report.sql", "select 1;\n"),
        ("templates/weekly-report.j2", "hello {{ name }}\n"),
        ("skills/triage.md", "# Triage skill\n"),
        ("tools/foo/bar.py", "print('hi')\n"),
    ];

    for (relative, body) in cases {
        let abs = root.join(relative);
        fs::create_dir_all(abs.parent().unwrap()).unwrap();
        fs::write(&abs, body).unwrap();

        let rust = vaultli::infer::infer_frontmatter(&abs, &root).unwrap();
        let rust_value = Value::Object(rust);

        let code = format!(
            "import json; from vaultli.core import infer_frontmatter; \
             print(json.dumps(infer_frontmatter(r'{path}', r'{root}')))",
            path = abs.to_string_lossy(),
            root = root.to_string_lossy()
        );
        let Some(py) = run_python(&code) else {
            panic!("python infer_frontmatter failed for {relative}");
        };

        // Both sides set today's date via UTC / local date; if they happen to
        // straddle midnight, ignore the date fields.
        let mut rust_trimmed = rust_value.clone();
        let mut py_trimmed = py.clone();
        for field in ["created", "updated"] {
            if let Some(obj) = rust_trimmed.as_object_mut() {
                obj.remove(field);
            }
            if let Some(obj) = py_trimmed.as_object_mut() {
                obj.remove(field);
            }
        }

        assert_eq!(
            rust_trimmed, py_trimmed,
            "infer mismatch for {relative}:\n rust={rust_value}\n  py={py}"
        );
    }
}

#[test]
fn index_matches_python_for_mixed_vault() {
    if !python_available() {
        eprintln!("python vaultli not importable; skipping parity test");
        return;
    }

    let root = temp_dir("index");
    fs::write(root.join(VAULT_MARKER), "").unwrap();
    fs::write(root.join("INDEX.jsonl"), "").unwrap();

    let good = root.join("docs/guide.md");
    fs::create_dir_all(good.parent().unwrap()).unwrap();
    fs::write(
        &good,
        "---\nid: docs/guide\ntitle: Guide\ndescription: A guide\n---\nBody\n",
    )
    .unwrap();

    let missing_required = root.join("docs/bad.md");
    fs::write(&missing_required, "---\nid: docs/bad\n---\nBody\n").unwrap();

    let sidecar = root.join("queries/report.sql.md");
    fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
    fs::write(
        &sidecar,
        "---\nid: queries/report\ntitle: R\ndescription: D\n---\nBody\n",
    )
    .unwrap();

    vaultli::index::build_index(&root, true).unwrap();
    let rust_index = fs::read_to_string(root.join("INDEX.jsonl")).unwrap();

    // Reset and run Python build
    fs::write(root.join("INDEX.jsonl"), "").unwrap();
    let code = format!(
        "import json; from vaultli.core import build_index; r = build_index(r'{root}', full=True); print(json.dumps(r.to_dict()))",
        root = root.to_string_lossy()
    );
    let _ = run_python(&code).expect("python build_index failed");
    let py_index = fs::read_to_string(root.join("INDEX.jsonl")).unwrap();

    let rust_records: Vec<Value> = rust_index
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let py_records: Vec<Value> = py_index
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    assert_eq!(rust_records.len(), py_records.len());
    for (r, p) in rust_records.iter().zip(py_records.iter()) {
        assert_eq!(r["id"], p["id"]);
        assert_eq!(r["title"], p["title"]);
        assert_eq!(r["description"], p["description"]);
        assert_eq!(r["hash"], p["hash"], "hash mismatch for {}", r["id"]);
        assert_eq!(r["file"], p["file"]);
    }
}
