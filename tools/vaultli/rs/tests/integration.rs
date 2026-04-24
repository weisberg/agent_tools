use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use vaultli::id::make_id;
use vaultli::index::{build_index, parse_markdown_file};
use vaultli::infer::infer_frontmatter;
use vaultli::paths::find_root;
use vaultli::scaffold::{add_file, ingest_path, init_vault, scaffold_file};
use vaultli::validate::validate_vault;

const VAULT_MARKER: &str = ".kbroot";
const INDEX_FILENAME: &str = "INDEX.jsonl";

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("vaultli-rs-{name}-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn finds_root_upwards() {
    let root = temp_dir("root");
    fs::write(root.join(VAULT_MARKER), "").unwrap();
    let nested = root.join("docs/notes");
    fs::create_dir_all(&nested).unwrap();
    assert_eq!(
        find_root(Some(&nested)).unwrap(),
        root.canonicalize().unwrap()
    );
}

#[test]
fn derives_ids_for_sidecars() {
    let root = temp_dir("id");
    let file = root.join("queries/report.sql.md");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "").unwrap();
    assert_eq!(make_id(&file, &root).unwrap(), "queries/report");
}

#[test]
fn parses_markdown_and_indexes() {
    let root = temp_dir("index");
    init_vault(&root).unwrap();
    let doc = root.join("docs/guide.md");
    fs::create_dir_all(doc.parent().unwrap()).unwrap();
    fs::write(
        &doc,
        "---\nid: docs/guide\ntitle: Guide\ndescription: Helpful guide\n---\nBody\n",
    )
    .unwrap();
    let parsed = parse_markdown_file(&doc, &root).unwrap();
    assert!(parsed.has_frontmatter);
    assert_eq!(parsed.doc_id(), Some("docs/guide"));
    let result = build_index(&root, true).unwrap();
    assert_eq!(result.indexed, 1);
    let index_text = fs::read_to_string(root.join(INDEX_FILENAME)).unwrap();
    assert!(index_text.contains("\"id\":\"docs/guide\""));
}

#[test]
fn infers_frontmatter_for_templates() {
    let root = temp_dir("infer");
    init_vault(&root).unwrap();
    let template = root.join("templates/report.j2");
    fs::create_dir_all(template.parent().unwrap()).unwrap();
    fs::write(&template, "hello {{ name }}").unwrap();
    let inferred = infer_frontmatter(&template, &root).unwrap();
    assert_eq!(
        inferred.get("category"),
        Some(&Value::String("template".into()))
    );
    assert_eq!(
        inferred.get("source"),
        Some(&Value::String("./report.j2".into()))
    );
}

#[test]
fn scaffolds_sidecar_and_adds_markdown() {
    let root = temp_dir("scaffold");
    init_vault(&root).unwrap();
    let sql = root.join("queries/report.sql");
    fs::create_dir_all(sql.parent().unwrap()).unwrap();
    fs::write(&sql, "select 1;").unwrap();
    let scaffolded = scaffold_file(&root, &sql).unwrap();
    assert_eq!(
        scaffolded.get("mode"),
        Some(&Value::String("sidecar".into()))
    );
    assert!(root.join("queries/report.sql.md").exists());

    let md = root.join("docs/notes.md");
    fs::create_dir_all(md.parent().unwrap()).unwrap();
    fs::write(&md, "# Notes\n").unwrap();
    let added = add_file(&root, &md).unwrap();
    assert_eq!(
        added.get("mode"),
        Some(&Value::String("frontmatter".into()))
    );
    let contents = fs::read_to_string(&md).unwrap();
    assert!(contents.starts_with("---\n"));
}

#[test]
fn ingests_directory_with_dry_run_and_indexing() {
    let root = temp_dir("ingest");
    init_vault(&root).unwrap();

    let notes = root.join("docs/notes.md");
    fs::create_dir_all(notes.parent().unwrap()).unwrap();
    fs::write(&notes, "# Notes\n").unwrap();

    let sql = root.join("queries/report.sql");
    fs::create_dir_all(sql.parent().unwrap()).unwrap();
    fs::write(&sql, "select 1;").unwrap();

    let dry_run = ingest_path(&root, &root, false, true).unwrap();
    assert_eq!(dry_run.get("dry_run"), Some(&Value::Bool(true)));
    assert_eq!(dry_run.get("indexed"), Some(&Value::Bool(false)));
    assert!(!root.join("queries/report.sql.md").exists());

    let scaffolded = dry_run.get("scaffolded").and_then(Value::as_array).unwrap();
    let files = scaffolded
        .iter()
        .map(|entry| entry.get("file").and_then(Value::as_str).unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        files,
        BTreeSet::from(["docs/notes.md", "queries/report.sql.md"])
    );

    let ingested = ingest_path(&root, &root, true, false).unwrap();
    assert_eq!(ingested.get("indexed"), Some(&Value::Bool(true)));
    assert!(root.join("queries/report.sql.md").exists());
    assert!(fs::read_to_string(&notes).unwrap().starts_with("---\n"));
}

#[test]
fn validates_broken_sources_and_duplicates() {
    let root = temp_dir("validate");
    init_vault(&root).unwrap();
    let doc1 = root.join("docs/one.md");
    fs::create_dir_all(doc1.parent().unwrap()).unwrap();
    fs::write(
        &doc1,
        "---\nid: docs/dup\ntitle: One\ndescription: First\ndepends_on:\n  - docs/missing\n---\nBody\n",
    )
    .unwrap();
    let doc2 = root.join("docs/two.md");
    fs::write(
        &doc2,
        "---\nid: docs/dup\ntitle: Two\ndescription: Second\n---\nBody\n",
    )
    .unwrap();
    let broken = root.join("queries/broken.sql.md");
    fs::create_dir_all(broken.parent().unwrap()).unwrap();
    fs::write(
        &broken,
        "---\nid: queries/broken\ntitle: Broken\ndescription: Broken\nsource: ./broken.sql\n---\nBody\n",
    )
    .unwrap();
    build_index(&root, true).unwrap();
    let validation = validate_vault(&root).unwrap();
    let codes = validation
        .issues
        .iter()
        .map(|issue| issue.code.clone())
        .collect::<BTreeSet<_>>();
    assert!(codes.contains("BROKEN_SOURCE"));
    assert!(codes.contains("ORPHANED_SIDECAR"));
    assert!(codes.contains("DUPLICATE_ID"));
    assert!(codes.contains("DANGLING_DEPENDENCY"));
}
