mod common;

use common::bin;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn pair(old: &str, new: &str) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempdir().unwrap();
    let old_path = dir.path().join("old.md");
    let new_path = dir.path().join("new.md");
    fs::write(&old_path, old).unwrap();
    fs::write(&new_path, new).unwrap();
    (dir, old_path, new_path)
}

#[test]
fn diff_identical_documents_reports_no_changes() {
    let (_dir, old, new) = pair(
        "<!-- mdli:id v=1 id=a -->\n# A\n",
        "<!-- mdli:id v=1 id=a -->\n# A\n",
    );
    bin()
        .args([
            "diff",
            new.to_str().unwrap(),
            "--against",
            old.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"findings\": []"))
        .stdout(predicate::str::contains("\"sections_added\": 0"));
}

#[test]
fn diff_text_mode_summarizes_no_change() {
    let (_dir, old, new) = pair("# A\n", "# A\n");
    bin()
        .args([
            "diff",
            new.to_str().unwrap(),
            "--against",
            old.to_str().unwrap(),
            "--text",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("no semantic changes"));
}

#[test]
fn diff_detects_section_added() {
    let (_dir, old, new) = pair(
        "<!-- mdli:id v=1 id=a -->\n# A\n",
        "<!-- mdli:id v=1 id=a -->\n# A\n\n<!-- mdli:id v=1 id=b -->\n## B\n",
    );
    bin()
        .args([
            "diff",
            new.to_str().unwrap(),
            "--against",
            old.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"section.added\""))
        .stdout(predicate::str::contains("\"id\": \"b\""));
}

#[test]
fn diff_detects_section_removed() {
    let (_dir, old, new) = pair(
        "<!-- mdli:id v=1 id=a -->\n# A\n\n<!-- mdli:id v=1 id=b -->\n## B\n",
        "<!-- mdli:id v=1 id=a -->\n# A\n",
    );
    bin()
        .args([
            "diff",
            new.to_str().unwrap(),
            "--against",
            old.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"section.removed\""));
}

#[test]
fn diff_detects_section_renamed_via_stable_id() {
    let (_dir, old, new) = pair(
        "<!-- mdli:id v=1 id=intro -->\n## Intro\n",
        "<!-- mdli:id v=1 id=intro -->\n## Introduction\n",
    );
    bin()
        .args([
            "diff",
            new.to_str().unwrap(),
            "--against",
            old.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"section.renamed\""))
        .stdout(predicate::str::contains("\"old_title\": \"Intro\""))
        .stdout(predicate::str::contains("\"new_title\": \"Introduction\""));
}

#[test]
fn diff_detects_section_moved_when_parent_changes() {
    let (_dir, old, new) = pair(
        "# Top\n\n## Parent A\n\n<!-- mdli:id v=1 id=child -->\n### Child\n\n## Parent B\n",
        "# Top\n\n## Parent A\n\n## Parent B\n\n<!-- mdli:id v=1 id=child -->\n### Child\n",
    );
    bin()
        .args([
            "diff",
            new.to_str().unwrap(),
            "--against",
            old.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"section.moved\""))
        .stdout(predicate::str::contains("Parent A"))
        .stdout(predicate::str::contains("Parent B"));
}

#[test]
fn diff_detects_section_level_changed() {
    let (_dir, old, new) = pair(
        "<!-- mdli:id v=1 id=foo -->\n## Foo\n",
        "<!-- mdli:id v=1 id=foo -->\n### Foo\n",
    );
    bin()
        .args([
            "diff",
            new.to_str().unwrap(),
            "--against",
            old.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"section.level_changed\""))
        .stdout(predicate::str::contains("\"old_level\": 2"))
        .stdout(predicate::str::contains("\"new_level\": 3"));
}

#[test]
fn diff_detects_table_added_and_removed() {
    let (_dir, old, new) = pair(
        "<!-- mdli:id v=1 id=t -->\n## T\n",
        "<!-- mdli:id v=1 id=t -->\n## T\n\n<!-- mdli:table v=1 name=foo -->\n| A |\n| --- |\n| 1 |\n",
    );
    bin()
        .args([
            "diff",
            new.to_str().unwrap(),
            "--against",
            old.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"table.added\""))
        .stdout(predicate::str::contains("\"name\": \"foo\""));

    let (_dir, old2, new2) = pair(
        "<!-- mdli:id v=1 id=t -->\n## T\n\n<!-- mdli:table v=1 name=foo -->\n| A |\n| --- |\n| 1 |\n",
        "<!-- mdli:id v=1 id=t -->\n## T\n",
    );
    bin()
        .args([
            "diff",
            new2.to_str().unwrap(),
            "--against",
            old2.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"table.removed\""));
}

#[test]
fn diff_detects_table_rows_added_removed_and_updated_by_key() {
    let old = "<!-- mdli:id v=1 id=t -->\n## T\n\n<!-- mdli:table v=1 name=tt key=K -->\n| K | V |\n| --- | --- |\n| 1 | one |\n| 2 | two |\n";
    let new = "<!-- mdli:id v=1 id=t -->\n## T\n\n<!-- mdli:table v=1 name=tt key=K -->\n| K | V |\n| --- | --- |\n| 1 | one |\n| 2 | TWO |\n| 3 | three |\n";
    let (_dir, old_p, new_p) = pair(old, new);
    bin()
        .args([
            "diff",
            new_p.to_str().unwrap(),
            "--against",
            old_p.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"table.rows_changed\""))
        .stdout(predicate::str::contains("\"key\": \"K\""))
        .stdout(predicate::str::contains("\"rows_added\": 1"))
        .stdout(predicate::str::contains("\"rows_updated\": 1"))
        .stdout(predicate::str::contains("\"rows_removed\": 0"));
}

#[test]
fn diff_detects_table_column_shape_change() {
    let old = "<!-- mdli:id v=1 id=t -->\n## T\n\n<!-- mdli:table v=1 name=tt -->\n| A | B |\n| --- | --- |\n| 1 | 2 |\n";
    let new = "<!-- mdli:id v=1 id=t -->\n## T\n\n<!-- mdli:table v=1 name=tt -->\n| A | B | C |\n| --- | --- | --- |\n| 1 | 2 | 3 |\n";
    let (_dir, old_p, new_p) = pair(old, new);
    bin()
        .args([
            "diff",
            new_p.to_str().unwrap(),
            "--against",
            old_p.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"table.columns_changed\""));
}

#[test]
fn diff_detects_block_added_removed_and_content_changed() {
    let (_dir, old, new) = pair(
        "<!-- mdli:id v=1 id=s -->\n## S\n\n<!-- mdli:begin v=1 id=s.gen checksum=sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 -->\n<!-- mdli:end v=1 id=s.gen -->\n",
        "<!-- mdli:id v=1 id=s -->\n## S\n\n<!-- mdli:begin v=1 id=s.gen checksum=sha256:abc -->\nNew body.\n<!-- mdli:end v=1 id=s.gen -->\n\n<!-- mdli:begin v=1 id=s.note checksum=sha256:def -->\nNote.\n<!-- mdli:end v=1 id=s.note -->\n",
    );
    bin()
        .args([
            "diff",
            new.to_str().unwrap(),
            "--against",
            old.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"block.content_changed\""))
        .stdout(predicate::str::contains("\"block.added\""))
        .stdout(predicate::str::contains("s.note"));
}

#[test]
fn diff_flags_locked_edit_attempted_when_locked_block_content_changes() {
    let old = "<!-- mdli:id v=1 id=s -->\n## S\n\n<!-- mdli:begin v=1 id=s.gen checksum=sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 locked=true -->\n<!-- mdli:end v=1 id=s.gen -->\n";
    let new = "<!-- mdli:id v=1 id=s -->\n## S\n\n<!-- mdli:begin v=1 id=s.gen checksum=sha256:abc locked=true -->\nTampered.\n<!-- mdli:end v=1 id=s.gen -->\n";
    let (_dir, old_p, new_p) = pair(old, new);
    bin()
        .args([
            "diff",
            new_p.to_str().unwrap(),
            "--against",
            old_p.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"block.locked_edit_attempted\""));
}

#[test]
fn diff_detects_block_lock_state_change() {
    let old = "<!-- mdli:id v=1 id=s -->\n## S\n\n<!-- mdli:begin v=1 id=s.gen checksum=sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 -->\n<!-- mdli:end v=1 id=s.gen -->\n";
    let new = "<!-- mdli:id v=1 id=s -->\n## S\n\n<!-- mdli:begin v=1 id=s.gen checksum=sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 locked=true -->\n<!-- mdli:end v=1 id=s.gen -->\n";
    let (_dir, old_p, new_p) = pair(old, new);
    bin()
        .args([
            "diff",
            new_p.to_str().unwrap(),
            "--against",
            old_p.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"block.lock_changed\""))
        .stdout(predicate::str::contains("\"old_locked\": false"))
        .stdout(predicate::str::contains("\"new_locked\": true"));
}

#[test]
fn diff_detects_frontmatter_add_remove_and_change() {
    let old = "---\ntitle: Old\nstatus: draft\n---\n\n# A\n";
    let new = "---\ntitle: New\nowner: alex\n---\n\n# A\n";
    let (_dir, old_p, new_p) = pair(old, new);
    bin()
        .args([
            "diff",
            new_p.to_str().unwrap(),
            "--against",
            old_p.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"frontmatter.changed\""))
        .stdout(predicate::str::contains("\"frontmatter.added\""))
        .stdout(predicate::str::contains("\"frontmatter.removed\""));
}

#[test]
fn diff_text_mode_renders_summary_and_findings() {
    let (_dir, old, new) = pair(
        "<!-- mdli:id v=1 id=intro -->\n## Intro\n",
        "<!-- mdli:id v=1 id=intro -->\n## Introduction\n",
    );
    bin()
        .args([
            "diff",
            new.to_str().unwrap(),
            "--against",
            old.to_str().unwrap(),
            "--text",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("summary:"))
        .stdout(predicate::str::contains("sections_renamed: 1"))
        .stdout(predicate::str::contains("findings:"))
        .stdout(predicate::str::contains("renamed"));
}
