mod common;

use common::bin;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

const FULL_DOC: &str = "<!-- mdli:id v=1 id=cashplus.okr -->\n## OKR\n\n<!-- mdli:id v=1 id=cashplus.analytics -->\n## Analytics\n\n<!-- mdli:table v=1 name=analytics-tickets key=Ticket -->\n| Ticket | Summary | Status |\n| --- | --- | --- |\n| CP-1 | First | Open |\n\n<!-- mdli:begin v=1 id=cashplus.analytics.generated checksum=sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 -->\n<!-- mdli:end v=1 id=cashplus.analytics.generated -->\n";

const FULL_SCHEMA: &str = "schema: mdli/validation/v1\nrequired_sections:\n  - id: cashplus.okr\n    level: 2\n  - id: cashplus.analytics\n    level: 2\nrequired_tables:\n  - name: analytics-tickets\n    columns: [Ticket, Summary, Status]\n    key: Ticket\nmanaged_blocks:\n  - id: cashplus.analytics.generated\n    locked: false\n";

#[test]
fn validate_passes_when_document_matches_schema() {
    let dir = tempdir().unwrap();
    let doc = dir.path().join("doc.md");
    let schema = dir.path().join("schema.yml");
    fs::write(&doc, FULL_DOC).unwrap();
    fs::write(&schema, FULL_SCHEMA).unwrap();

    bin()
        .args([
            "validate",
            doc.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ok\": true"))
        .stdout(predicate::str::contains("\"findings\": []"));
}

#[test]
fn validate_reports_missing_section() {
    let dir = tempdir().unwrap();
    let doc = dir.path().join("doc.md");
    let schema = dir.path().join("schema.yml");
    fs::write(&doc, "<!-- mdli:id v=1 id=cashplus.okr -->\n## OKR\n").unwrap();
    fs::write(&schema, FULL_SCHEMA).unwrap();

    bin()
        .args([
            "validate",
            doc.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ok\": false"))
        .stdout(predicate::str::contains("E_VALIDATION_MISSING_SECTION"))
        .stdout(predicate::str::contains("cashplus.analytics"));
}

#[test]
fn validate_reports_missing_table() {
    let dir = tempdir().unwrap();
    let doc = dir.path().join("doc.md");
    let schema = dir.path().join("schema.yml");
    fs::write(
        &doc,
        "<!-- mdli:id v=1 id=cashplus.okr -->\n## OKR\n\n<!-- mdli:id v=1 id=cashplus.analytics -->\n## Analytics\n\n<!-- mdli:begin v=1 id=cashplus.analytics.generated checksum=sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 -->\n<!-- mdli:end v=1 id=cashplus.analytics.generated -->\n",
    )
    .unwrap();
    fs::write(&schema, FULL_SCHEMA).unwrap();

    bin()
        .args([
            "validate",
            doc.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("E_VALIDATION_MISSING_TABLE"))
        .stdout(predicate::str::contains("analytics-tickets"));
}

#[test]
fn validate_reports_table_column_mismatch() {
    let dir = tempdir().unwrap();
    let doc = dir.path().join("doc.md");
    let schema = dir.path().join("schema.yml");
    fs::write(
        &doc,
        "<!-- mdli:id v=1 id=cashplus.okr -->\n## OKR\n\n<!-- mdli:id v=1 id=cashplus.analytics -->\n## Analytics\n\n<!-- mdli:table v=1 name=analytics-tickets key=Ticket -->\n| Ticket | Summary |\n| --- | --- |\n| CP-1 | First |\n\n<!-- mdli:begin v=1 id=cashplus.analytics.generated checksum=sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 -->\n<!-- mdli:end v=1 id=cashplus.analytics.generated -->\n",
    )
    .unwrap();
    fs::write(&schema, FULL_SCHEMA).unwrap();

    bin()
        .args([
            "validate",
            doc.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("E_VALIDATION_TABLE_COLUMNS"));
}

#[test]
fn validate_reports_locked_state_mismatch() {
    let dir = tempdir().unwrap();
    let doc = dir.path().join("doc.md");
    let schema = dir.path().join("schema.yml");
    fs::write(
        &doc,
        "<!-- mdli:id v=1 id=cashplus.okr -->\n## OKR\n\n<!-- mdli:id v=1 id=cashplus.analytics -->\n## Analytics\n\n<!-- mdli:table v=1 name=analytics-tickets key=Ticket -->\n| Ticket | Summary | Status |\n| --- | --- | --- |\n| CP-1 | First | Open |\n\n<!-- mdli:begin v=1 id=cashplus.analytics.generated checksum=sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 locked=true -->\n<!-- mdli:end v=1 id=cashplus.analytics.generated -->\n",
    )
    .unwrap();
    fs::write(&schema, FULL_SCHEMA).unwrap();

    bin()
        .args([
            "validate",
            doc.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("E_VALIDATION_BLOCK_LOCK"));
}

#[test]
fn validate_rejects_wrong_schema_version() {
    let dir = tempdir().unwrap();
    let doc = dir.path().join("doc.md");
    let schema = dir.path().join("schema.yml");
    fs::write(&doc, "# Doc\n").unwrap();
    fs::write(&schema, "schema: not/a/known/schema\n").unwrap();

    bin()
        .args([
            "validate",
            doc.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_VALIDATION_SCHEMA_INVALID"));
}
