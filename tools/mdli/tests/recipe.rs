mod common;

use common::bin;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn write_basic_recipe(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let recipe = dir.join("recipe.yml");
    fs::write(
        &recipe,
        "schema: mdli/recipe/v1\ntitle: Demo\nsections:\n  - id: report.analytics\n    path: \"Report > Analytics\"\n    level: 2\n    template: templates/analytics.mdli\n    bindings:\n      tickets: tickets\n",
    )
    .unwrap();
    let templates = dir.join("templates");
    fs::create_dir_all(&templates).unwrap();
    let tpl = templates.join("analytics.mdli");
    fs::write(
        &tpl,
        "{{ table tickets columns=[\"Ticket=key\",\"Summary=summary\"] key=\"Ticket\" }}\n",
    )
    .unwrap();
    (recipe, tpl)
}

fn write_rows(dir: &std::path::Path) -> std::path::PathBuf {
    let rows = dir.join("tickets.ndjson");
    fs::write(
        &rows,
        "{\"key\":\"CP-1\",\"summary\":\"First\"}\n{\"key\":\"CP-2\",\"summary\":\"Second\"}\n",
    )
    .unwrap();
    rows
}

#[test]
fn recipe_validate_accepts_yaml_recipe() {
    let dir = tempdir().unwrap();
    let (recipe, _) = write_basic_recipe(dir.path());

    bin()
        .args(["recipe", "validate", recipe.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema\": \"mdli/recipe/v1\""))
        .stdout(predicate::str::contains("report.analytics"));
}

#[test]
fn recipe_validate_rejects_wrong_schema() {
    let dir = tempdir().unwrap();
    let recipe = dir.path().join("bad.yml");
    fs::write(&recipe, "schema: not/a/known/schema\nsections: []\n").unwrap();

    bin()
        .args(["recipe", "validate", recipe.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_RECIPE_INVALID"));
}

#[test]
fn apply_creates_section_block_and_table_end_to_end() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    fs::write(&report, "# Report\n\n").unwrap();
    let (recipe, _) = write_basic_recipe(dir.path());
    let rows = write_rows(dir.path());

    bin()
        .args([
            "apply",
            report.to_str().unwrap(),
            "--recipe",
            recipe.to_str().unwrap(),
            "--data",
            &format!("tickets={}", rows.display()),
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&report).unwrap();
    assert!(out.contains("<!-- mdli:id v=1 id=report.analytics -->"));
    assert!(out.contains("## Analytics"));
    assert!(out.contains("<!-- mdli:begin v=1 id=report.analytics.generated"));
    assert!(out.contains("| CP-1   | First   |"));
}

#[test]
fn apply_is_idempotent() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    fs::write(&report, "# Report\n\n").unwrap();
    let (recipe, _) = write_basic_recipe(dir.path());
    let rows = write_rows(dir.path());

    bin()
        .args([
            "apply",
            report.to_str().unwrap(),
            "--recipe",
            recipe.to_str().unwrap(),
            "--data",
            &format!("tickets={}", rows.display()),
            "--write",
        ])
        .assert()
        .success();
    let first = fs::read_to_string(&report).unwrap();

    bin()
        .args([
            "apply",
            report.to_str().unwrap(),
            "--recipe",
            recipe.to_str().unwrap(),
            "--data",
            &format!("tickets={}", rows.display()),
            "--write",
        ])
        .assert()
        .success();
    let second = fs::read_to_string(&report).unwrap();

    assert_eq!(first, second, "apply must be idempotent");
}

#[test]
fn plan_then_apply_plan_matches_direct_apply() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    fs::write(&report, "# Report\n\n").unwrap();
    let report_via_apply = dir.path().join("via-apply.md");
    fs::copy(&report, &report_via_apply).unwrap();
    let (recipe, _) = write_basic_recipe(dir.path());
    let rows = write_rows(dir.path());
    let plan = dir.path().join("plan.json");

    let plan_out = bin()
        .args([
            "plan",
            report.to_str().unwrap(),
            "--recipe",
            recipe.to_str().unwrap(),
            "--data",
            &format!("tickets={}", rows.display()),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&plan, plan_out).unwrap();

    bin()
        .args([
            "apply-plan",
            report.to_str().unwrap(),
            "--plan",
            plan.to_str().unwrap(),
            "--write",
        ])
        .assert()
        .success();

    bin()
        .args([
            "apply",
            report_via_apply.to_str().unwrap(),
            "--recipe",
            recipe.to_str().unwrap(),
            "--data",
            &format!("tickets={}", rows.display()),
            "--write",
        ])
        .assert()
        .success();

    // The two paths should produce equivalent ID/heading/table content. Recipe
    // provenance differs (apply records the recipe hash; apply-plan does not),
    // so we compare structural content excluding the begin marker.
    let from_plan = fs::read_to_string(&report).unwrap();
    let from_apply = fs::read_to_string(&report_via_apply).unwrap();
    let strip = |s: &str| {
        s.lines()
            .filter(|l| !l.contains("mdli:begin"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip(&from_plan), strip(&from_apply));
}

#[test]
fn apply_plan_rejects_stale_preimage() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    fs::write(&report, "# Report\n\n").unwrap();
    let (recipe, _) = write_basic_recipe(dir.path());
    let rows = write_rows(dir.path());
    let plan = dir.path().join("plan.json");

    let plan_out = bin()
        .args([
            "plan",
            report.to_str().unwrap(),
            "--recipe",
            recipe.to_str().unwrap(),
            "--data",
            &format!("tickets={}", rows.display()),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&plan, plan_out).unwrap();

    // Mutate the document so its hash no longer matches the plan preimage.
    fs::write(&report, "# Different\n").unwrap();

    bin()
        .args([
            "apply-plan",
            report.to_str().unwrap(),
            "--plan",
            plan.to_str().unwrap(),
            "--write",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_STALE_PREIMAGE"));
}

#[test]
fn build_creates_new_document_from_recipe() {
    let dir = tempdir().unwrap();
    let (recipe, _) = write_basic_recipe(dir.path());
    let rows = write_rows(dir.path());
    let out = dir.path().join("built.md");

    bin()
        .args([
            "build",
            "--recipe",
            recipe.to_str().unwrap(),
            "--data",
            &format!("tickets={}", rows.display()),
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    let text = fs::read_to_string(&out).unwrap();
    assert!(text.contains("# Demo"));
    assert!(text.contains("## Analytics"));
    assert!(text.contains("CP-1"));
}

#[test]
fn build_refuses_existing_target_without_overwrite() {
    let dir = tempdir().unwrap();
    let (recipe, _) = write_basic_recipe(dir.path());
    let rows = write_rows(dir.path());
    let out = dir.path().join("built.md");
    fs::write(&out, "existing").unwrap();

    bin()
        .args([
            "build",
            "--recipe",
            recipe.to_str().unwrap(),
            "--data",
            &format!("tickets={}", rows.display()),
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_WRITE_FAILED"));
}

#[test]
fn patch_applies_ordered_edit_ops() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    fs::write(&report, "# Report\n\n").unwrap();
    let edits = dir.path().join("edits.json");
    fs::write(
        &edits,
        "[{\"op\":\"ensure_section\",\"id\":\"r.intro\",\"path\":\"Report > Intro\",\"level\":2},{\"op\":\"rename_section\",\"id\":\"r.intro\",\"title\":\"Introduction\"}]",
    )
    .unwrap();

    bin()
        .args([
            "patch",
            report.to_str().unwrap(),
            "--edits",
            edits.to_str().unwrap(),
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&report).unwrap();
    assert!(out.contains("## Introduction"));
    assert!(out.contains("id=r.intro"));
}
