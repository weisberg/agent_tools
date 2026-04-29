mod common;

use common::bin;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn template_value_helper_substitutes_scalar() {
    let dir = tempdir().unwrap();
    let template = dir.path().join("t.mdli");
    let value = dir.path().join("v.json");
    fs::write(&template, "**Updated:** {{ value updated }}\n").unwrap();
    fs::write(&value, "\"2026-04-27\"").unwrap();

    bin()
        .args([
            "template",
            "render",
            template.to_str().unwrap(),
            "--data",
            &format!("updated={}", value.display()),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("**Updated:** 2026-04-27"));
}

#[test]
fn template_table_helper_renders_named_table() {
    let dir = tempdir().unwrap();
    let template = dir.path().join("t.mdli");
    let rows = dir.path().join("rows.ndjson");
    fs::write(
        &template,
        "Hello\n\n{{ table tickets columns=[\"Ticket=key\",\"Summary=summary\"] key=\"Ticket\" }}\n",
    )
    .unwrap();
    fs::write(
        &rows,
        "{\"key\":\"CP-1\",\"summary\":\"First\"}\n{\"key\":\"CP-2\",\"summary\":\"Second\"}\n",
    )
    .unwrap();

    bin()
        .args([
            "template",
            "render",
            template.to_str().unwrap(),
            "--data",
            &format!("tickets={}", rows.display()),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("| Ticket | Summary |"))
        .stdout(predicate::str::contains("| CP-1   | First   |"))
        .stdout(predicate::str::contains("| CP-2   | Second  |"));
}

#[test]
fn template_if_present_block_is_skipped_when_dataset_absent() {
    let dir = tempdir().unwrap();
    let template = dir.path().join("t.mdli");
    fs::write(
        &template,
        "Always\n{{ if_present optional }}OPTIONAL{{ end }}\n",
    )
    .unwrap();

    bin()
        .args(["template", "render", template.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Always"))
        .stdout(predicate::str::contains("OPTIONAL").not());
}

#[test]
fn template_missing_dataset_errors() {
    let dir = tempdir().unwrap();
    let template = dir.path().join("t.mdli");
    fs::write(&template, "{{ table missing columns=[\"X=x\"] }}\n").unwrap();

    bin()
        .args(["template", "render", template.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_TEMPLATE_MISSING_DATASET"));
}

#[test]
fn template_unknown_helper_errors() {
    let dir = tempdir().unwrap();
    let template = dir.path().join("t.mdli");
    fs::write(&template, "{{ bogus arg }}\n").unwrap();

    bin()
        .args(["template", "render", template.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_TEMPLATE_UNKNOWN_HELPER"));
}

#[test]
fn template_unterminated_helper_errors() {
    let dir = tempdir().unwrap();
    let template = dir.path().join("t.mdli");
    fs::write(&template, "{{ value oops").unwrap();

    bin()
        .args(["template", "render", template.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_TEMPLATE_PARSE"));
}
