use assert_cmd::Command;
use predicates::prelude::*;

fn clipli() -> Command {
    Command::cargo_bin("clipli").unwrap()
}

// ---------------------------------------------------------------------------
// 1. list succeeds (even if the store is empty or has templates)
// ---------------------------------------------------------------------------

#[test]
fn test_list_succeeds() {
    clipli()
        .arg("list")
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// 2. list --json succeeds and outputs valid JSON array
// ---------------------------------------------------------------------------

#[test]
fn test_list_json_succeeds() {
    let output = clipli()
        .args(["list", "--json"])
        .assert()
        .success();

    let raw = output.get_output().stdout.clone();
    let stdout = String::from_utf8(raw).unwrap();
    let trimmed = stdout.trim_start();
    assert!(
        trimmed.starts_with('['),
        "expected JSON array, got: {}",
        &stdout[..stdout.len().min(80)]
    );
}

// ---------------------------------------------------------------------------
// 3. list with a nonexistent tag filter returns success (empty list)
// ---------------------------------------------------------------------------

#[test]
fn test_list_with_tag_filter() {
    clipli()
        .args(["list", "--tag", "nonexistent_tag_xyz"])
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// 4. delete a nonexistent template with --force fails with "not found"
// ---------------------------------------------------------------------------

#[test]
fn test_delete_nonexistent_force() {
    clipli()
        .args(["delete", "nonexistent_template_xyz_12345", "--force"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// ---------------------------------------------------------------------------
// 5. show a nonexistent template fails with "not found"
// ---------------------------------------------------------------------------

#[test]
fn test_show_nonexistent() {
    clipli()
        .args(["show", "nonexistent_template_xyz_12345"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// ---------------------------------------------------------------------------
// 6. delete with no name argument fails (clap missing-arg error)
// ---------------------------------------------------------------------------

#[test]
fn test_delete_requires_name() {
    clipli()
        .arg("delete")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

// ---------------------------------------------------------------------------
// 7. show with no name argument fails (clap missing-arg error)
// ---------------------------------------------------------------------------

#[test]
fn test_show_requires_name() {
    clipli()
        .arg("show")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

// ---------------------------------------------------------------------------
// 8. capture rejects path-traversal names
// ---------------------------------------------------------------------------

#[test]
fn test_capture_validates_name() {
    clipli()
        .args(["capture", "--name", "../evil"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

// ---------------------------------------------------------------------------
// 9. capture rejects names with spaces
// ---------------------------------------------------------------------------

#[test]
fn test_capture_validates_name_with_spaces() {
    clipli()
        .args(["capture", "--name", "hello world"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

// ---------------------------------------------------------------------------
// 10. capture rejects names with dots
// ---------------------------------------------------------------------------

#[test]
fn test_capture_validates_name_with_dots() {
    clipli()
        .args(["capture", "--name", "foo.bar"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}
