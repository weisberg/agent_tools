use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use vaultli::context::assemble_context;
use vaultli::federation::federated_search;
use vaultli::gitinfo::git_info;
use vaultli::id::make_id;
use vaultli::index::{build_index, parse_markdown_file};
use vaultli::infer::infer_frontmatter;
use vaultli::metadata::{refresh_metadata, set_metadata_field, unset_metadata_field};
use vaultli::paths::find_root;
use vaultli::scaffold::{add_file, ingest_path, init_vault, scaffold_file};
use vaultli::search::{cat_record, resolve_record, search_records};
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

fn run_git(root: &PathBuf, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
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

    let dry_run = ingest_path(&root, &root, false, true, &[], &[]).unwrap();
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

    let ingested = ingest_path(&root, &root, true, false, &[], &[]).unwrap();
    assert_eq!(ingested.get("indexed"), Some(&Value::Bool(true)));
    assert!(root.join("queries/report.sql.md").exists());
    assert!(fs::read_to_string(&notes).unwrap().starts_with("---\n"));
}

#[test]
fn ingest_respects_include_and_exclude_patterns() {
    let root = temp_dir("ingest-patterns");
    init_vault(&root).unwrap();

    let notes = root.join("docs/notes.md");
    fs::create_dir_all(notes.parent().unwrap()).unwrap();
    fs::write(&notes, "# Notes\n").unwrap();
    let report = root.join("queries/report.sql");
    fs::create_dir_all(report.parent().unwrap()).unwrap();
    fs::write(&report, "select 1;").unwrap();
    fs::write(root.join("queries/skip.sql"), "select 2;").unwrap();

    let include = vec!["queries/*.sql".to_string()];
    let exclude = vec!["queries/skip.sql".to_string()];
    let result = ingest_path(&root, &root, false, true, &include, &exclude).unwrap();
    let scaffolded = result.get("scaffolded").and_then(Value::as_array).unwrap();
    let files = scaffolded
        .iter()
        .map(|entry| entry.get("file").and_then(Value::as_str).unwrap())
        .collect::<BTreeSet<_>>();

    assert_eq!(files, BTreeSet::from(["queries/report.sql.md"]));
    assert_eq!(
        result
            .get("include")
            .and_then(Value::as_array)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        result
            .get("exclude")
            .and_then(Value::as_array)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn searches_with_field_tag_filters_and_limit() {
    let root = temp_dir("search-filters");
    init_vault(&root).unwrap();

    let guide = root.join("docs/guide.md");
    fs::create_dir_all(guide.parent().unwrap()).unwrap();
    fs::write(
        &guide,
        "---\nid: docs/guide\ntitle: Guide\ndescription: Helpful guide\ncategory: reference\nstatus: active\ndomain: tooling\nscope: team\ntags:\n  - tooling\n  - onboarding\n---\nBody\n",
    )
    .unwrap();

    let draft = root.join("docs/draft.md");
    fs::write(
        &draft,
        "---\nid: docs/draft\ntitle: Draft\ndescription: Draft note\ncategory: note\nstatus: draft\ndomain: finance\nscope: personal\ntags:\n  - finance\n---\nBody\n",
    )
    .unwrap();

    build_index(&root, true).unwrap();
    let tags = vec!["tooling".to_string(), "onboarding".to_string()];
    let results = search_records(
        &root,
        None,
        None,
        Some("reference"),
        Some("active"),
        Some("tooling"),
        Some("team"),
        &tags,
        Some(1),
        None,
        "asc",
        false,
        false,
    )
    .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].get("id"),
        Some(&Value::String("docs/guide".into()))
    );
}

#[test]
fn resolves_cats_and_assembles_context() {
    let root = temp_dir("retrieval");
    init_vault(&root).unwrap();

    let sql = root.join("queries/report.sql");
    fs::create_dir_all(sql.parent().unwrap()).unwrap();
    fs::write(&sql, "select 1;\n").unwrap();
    fs::write(
        root.join("queries/report.sql.md"),
        "---\nid: queries/report\ntitle: Report Query\ndescription: SQL report query\nsource: ./report.sql\ndepends_on:\n  - docs/guide\ntokens: 4\n---\nSidecar notes.\n",
    )
    .unwrap();

    let guide = root.join("docs/guide.md");
    fs::create_dir_all(guide.parent().unwrap()).unwrap();
    fs::write(
        &guide,
        "---\nid: docs/guide\ntitle: Guide\ndescription: Helpful guide\ntokens: 3\n---\nGuide body.\n",
    )
    .unwrap();
    build_index(&root, true).unwrap();

    let resolved = resolve_record(&root, "queries/report", true, true).unwrap();
    assert_eq!(
        resolved.get("source_file"),
        Some(&Value::String("queries/report.sql".into()))
    );
    assert_eq!(
        resolved.get("body"),
        Some(&Value::String("Sidecar notes.\n".into()))
    );
    assert_eq!(
        resolved.get("source_content"),
        Some(&Value::String("select 1;\n".into()))
    );

    let body = cat_record(&root, "queries/report", false).unwrap();
    let source = cat_record(&root, "queries/report", true).unwrap();
    assert_eq!(
        body.get("content"),
        Some(&Value::String("Sidecar notes.\n".into()))
    );
    assert_eq!(
        source.get("content"),
        Some(&Value::String("select 1;\n".into()))
    );

    let context = assemble_context(
        &root,
        None,
        &["queries/report".to_string()],
        Some(10),
        false,
        true,
        None,
    )
    .unwrap();
    let records = context.get("records").and_then(Value::as_array).unwrap();
    assert_eq!(
        records
            .iter()
            .map(|entry| entry.get("id").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        vec!["queries/report", "docs/guide"]
    );

    let tight = assemble_context(
        &root,
        None,
        &["queries/report".to_string()],
        Some(3),
        false,
        true,
        None,
    )
    .unwrap();
    let tight_records = tight.get("records").and_then(Value::as_array).unwrap();
    assert_eq!(
        tight_records[0].get("id"),
        Some(&Value::String("docs/guide".into()))
    );

    fs::remove_file(sql).unwrap();
    assert!(cat_record(&root, "queries/report", true).is_err());
}

#[test]
fn federated_search_annotates_vault_origin() {
    let parent = temp_dir("federated");
    let first = parent.join("first");
    let second = parent.join("second");
    init_vault(&first).unwrap();
    init_vault(&second).unwrap();

    let first_doc = first.join("docs/guide.md");
    fs::create_dir_all(first_doc.parent().unwrap()).unwrap();
    fs::write(
        &first_doc,
        "---\nid: docs/guide\ntitle: First Guide\ndescription: shared alpha\n---\nFirst.\n",
    )
    .unwrap();
    let second_doc = second.join("docs/guide.md");
    fs::create_dir_all(second_doc.parent().unwrap()).unwrap();
    fs::write(
        &second_doc,
        "---\nid: docs/guide\ntitle: Second Guide\ndescription: shared alpha\n---\nSecond.\n",
    )
    .unwrap();
    build_index(&first, true).unwrap();
    build_index(&second, true).unwrap();

    let result = federated_search(
        &[first, second],
        Some("alpha"),
        None,
        None,
        false,
        false,
        None,
        "asc",
    )
    .unwrap();
    let records = result.get("results").and_then(Value::as_array).unwrap();
    let global_ids = records
        .iter()
        .map(|record| record.get("global_id").and_then(Value::as_str).unwrap())
        .collect::<BTreeSet<_>>();

    assert_eq!(result.get("total"), Some(&Value::from(2)));
    assert_eq!(
        global_ids,
        BTreeSet::from(["first:docs/guide", "second:docs/guide"])
    );
    assert!(federated_search(&[], None, None, None, false, false, None, "asc").is_err());
}

#[test]
fn git_info_reports_repo_and_file_state() {
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let root = temp_dir("git-info");
    init_vault(&root).unwrap();
    let doc = root.join("docs/guide.md");
    fs::create_dir_all(doc.parent().unwrap()).unwrap();
    fs::write(
        &doc,
        "---\nid: docs/guide\ntitle: Guide\ndescription: Helpful guide\n---\nGuide body.\n",
    )
    .unwrap();
    build_index(&root, true).unwrap();

    run_git(&root, &["init"]);
    run_git(&root, &["config", "user.email", "agent@example.com"]);
    run_git(&root, &["config", "user.name", "Agent"]);
    run_git(&root, &["add", "."]);
    run_git(&root, &["commit", "-m", "initial vault"]);

    let info = git_info(&root, Some("docs/guide")).unwrap();
    assert_eq!(info.get("available"), Some(&Value::Bool(true)));
    assert_eq!(
        info.get("file")
            .and_then(Value::as_object)
            .and_then(|file| file.get("tracked")),
        Some(&Value::Bool(true))
    );

    fs::write(
        &doc,
        "---\nid: docs/guide\ntitle: Guide\ndescription: Helpful guide\n---\nChanged.\n",
    )
    .unwrap();
    let dirty = git_info(&root, Some("docs/guide.md")).unwrap();
    assert_eq!(dirty.get("dirty"), Some(&Value::Bool(true)));
    assert_eq!(
        dirty
            .get("file")
            .and_then(Value::as_object)
            .and_then(|file| file.get("status"))
            .and_then(Value::as_str),
        Some(" M docs/guide.md")
    );
}

#[test]
fn git_info_is_safe_outside_git_repo() {
    let root = temp_dir("git-info-none");
    init_vault(&root).unwrap();
    let info = git_info(&root, None).unwrap();

    assert_eq!(info.get("available"), Some(&Value::Bool(false)));
    assert_eq!(
        info.get("reason"),
        Some(&Value::String("not a git repository".into()))
    );
}

#[test]
fn metadata_set_unset_refresh_and_defaults() {
    let root = temp_dir("metadata");
    init_vault(&root).unwrap();
    fs::write(
        root.join(VAULT_MARKER),
        "defaults:\n  author: brian\n  scope: team\n  domain: tooling\n",
    )
    .unwrap();
    let doc = root.join("docs/guide.md");
    fs::create_dir_all(doc.parent().unwrap()).unwrap();
    fs::write(
        &doc,
        "---\nid: docs/guide\ntitle: Old\ndescription: Old description\ntags:\n  - old\ncreated: 2026-01-01\n---\nImportant body\n",
    )
    .unwrap();
    build_index(&root, true).unwrap();

    let inferred = infer_frontmatter(&doc, &root).unwrap();
    assert_eq!(inferred.get("scope"), Some(&Value::String("team".into())));
    assert_eq!(
        inferred.get("domain"),
        Some(&Value::String("tooling".into()))
    );

    let set = set_metadata_field(&root, "docs/guide", "tags", "alpha,beta", true).unwrap();
    assert_eq!(set.get("value").and_then(Value::as_array).unwrap().len(), 2);

    let unset = unset_metadata_field(&root, "docs/guide", "tags", true).unwrap();
    assert_eq!(
        unset
            .get("removed")
            .and_then(Value::as_array)
            .unwrap()
            .len(),
        2
    );

    let refreshed = refresh_metadata(
        &root,
        "docs/guide",
        &["title".to_string(), "domain".to_string()],
        true,
    )
    .unwrap();
    assert_eq!(
        refreshed
            .get("fields")
            .and_then(Value::as_object)
            .and_then(|fields| fields.get("title")),
        Some(&Value::String("Guide".into()))
    );
    let parsed = parse_markdown_file(&doc, &root).unwrap();
    assert_eq!(
        parsed.metadata.get("created"),
        Some(&Value::String("2026-01-01".into()))
    );
    assert_eq!(parsed.body, "Important body\n");
    assert!(set_metadata_field(&root, "docs/guide", "priority", "not-an-int", false).is_err());
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
