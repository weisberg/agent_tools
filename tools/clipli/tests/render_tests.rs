use assert_cmd::Command;
use predicates::prelude::*;

fn clipli() -> Command {
    Command::cargo_bin("clipli").unwrap()
}

// ---------------------------------------------------------------------------
// 1. Jinja2 render — simple variable substitution
// ---------------------------------------------------------------------------

#[test]
fn test_j2_render_simple_variable() {
    clipli()
        .args(["convert", "--from", "j2", "--to", "html", "-D", r#"{"name":"World"}"#])
        .write_stdin("<p>Hello {{ name }}</p>")
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello World"));
}

// ---------------------------------------------------------------------------
// 2. Jinja2 render — multiple variables
// ---------------------------------------------------------------------------

#[test]
fn test_j2_render_multiple_variables() {
    clipli()
        .args([
            "convert",
            "--from",
            "j2",
            "--to",
            "html",
            "-D",
            r#"{"first":"John","last":"Doe"}"#,
        ])
        .write_stdin("<p>{{ first }} {{ last }}</p>")
        .assert()
        .success()
        .stdout(predicate::str::contains("John Doe"));
}

// ---------------------------------------------------------------------------
// 3. Jinja2 render — missing variable in lenient mode renders empty
// ---------------------------------------------------------------------------

#[test]
fn test_j2_render_missing_variable_lenient() {
    clipli()
        .args(["convert", "--from", "j2", "--to", "html"])
        .write_stdin("<p>Hello {{ name }}</p>")
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// 4. Jinja2 render — conditional (true)
// ---------------------------------------------------------------------------

#[test]
fn test_j2_render_conditional() {
    clipli()
        .args([
            "convert",
            "--from",
            "j2",
            "--to",
            "html",
            "-D",
            r#"{"show":true}"#,
        ])
        .write_stdin("{% if show %}<p>Visible</p>{% endif %}")
        .assert()
        .success()
        .stdout(predicate::str::contains("Visible"));
}

// ---------------------------------------------------------------------------
// 5. Jinja2 render — conditional (false)
// ---------------------------------------------------------------------------

#[test]
fn test_j2_render_conditional_false() {
    clipli()
        .args([
            "convert",
            "--from",
            "j2",
            "--to",
            "html",
            "-D",
            r#"{"show":false}"#,
        ])
        .write_stdin("{% if show %}<p>Visible</p>{% endif %}")
        .assert()
        .success()
        .stdout(predicate::str::contains("Visible").not());
}

// ---------------------------------------------------------------------------
// 6. Jinja2 render — loop
// ---------------------------------------------------------------------------

#[test]
fn test_j2_render_loop() {
    clipli()
        .args([
            "convert",
            "--from",
            "j2",
            "--to",
            "html",
            "-D",
            r#"{"items":["A","B","C"]}"#,
        ])
        .write_stdin("{% for item in items %}<li>{{ item }}</li>{% endfor %}")
        .assert()
        .success()
        .stdout(predicate::str::contains("<li>A</li>"))
        .stdout(predicate::str::contains("<li>B</li>"))
        .stdout(predicate::str::contains("<li>C</li>"));
}

// ---------------------------------------------------------------------------
// 7. Paste from table — dry-run outputs HTML table
// ---------------------------------------------------------------------------

#[test]
fn test_paste_from_table_dry_run() {
    let table_json = r##"{
  "headers": [{"value":"Name","style":{"bold":true}},{"value":"Score","style":{"bold":true}}],
  "rows": [[{"value":"Alice","style":{}},{"value":"95","style":{}}]],
  "style": {"header_bg":"#1F497D","header_fg":"#FFFFFF"}
}"##;

    clipli()
        .args(["paste", "--from-table", "--dry-run"])
        .write_stdin(table_json)
        .assert()
        .success()
        .stdout(predicate::str::contains("<table"))
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("95"))
        .stdout(predicate::str::contains("Name"))
        .stdout(predicate::str::contains("Score"));
}

// ---------------------------------------------------------------------------
// 8. Paste from table — striped template
// ---------------------------------------------------------------------------

#[test]
fn test_paste_from_table_with_template_striped() {
    let table_json = r##"{
  "headers": [{"value":"Name","style":{"bold":true}},{"value":"Score","style":{"bold":true}}],
  "rows": [[{"value":"Alice","style":{}},{"value":"95","style":{}}]],
  "style": {"header_bg":"#1F497D","header_fg":"#FFFFFF"}
}"##;

    clipli()
        .args(["paste", "--from-table", "--dry-run", "-t", "table_striped"])
        .write_stdin(table_json)
        .assert()
        .success()
        .stdout(predicate::str::contains("<table"));
}

// ---------------------------------------------------------------------------
// 9. HTML to plain text — list items
// ---------------------------------------------------------------------------

#[test]
fn test_html_to_plain_list_items() {
    clipli()
        .args(["convert", "--from", "html", "--to", "plain"])
        .write_stdin("<ul><li>First</li><li>Second</li></ul>")
        .assert()
        .success()
        .stdout(predicate::str::contains("First"))
        .stdout(predicate::str::contains("Second"));
}

// ---------------------------------------------------------------------------
// 10. HTML to plain text — table cells
// ---------------------------------------------------------------------------

#[test]
fn test_html_to_plain_table() {
    clipli()
        .args(["convert", "--from", "html", "--to", "plain"])
        .write_stdin("<table><tr><td>A</td><td>B</td></tr></table>")
        .assert()
        .success()
        .stdout(predicate::str::contains("A"))
        .stdout(predicate::str::contains("B"));
}
