use assert_cmd::Command;

fn clipli() -> Command {
    Command::cargo_bin("clipli").unwrap()
}

// ---------------------------------------------------------------------------
// 1. HTML -> J2 -> HTML round-trip with new data
// ---------------------------------------------------------------------------

#[test]
fn test_html_to_j2_to_html_round_trip() {
    // Step 1: templatize HTML into a Jinja2 template
    let step1 = clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>$1,234</td><td>Hello World</td>")
        .output()
        .unwrap();
    assert!(
        step1.status.success(),
        "step 1 (html->j2) failed: {}",
        String::from_utf8_lossy(&step1.stderr)
    );
    let j2_output = String::from_utf8(step1.stdout).unwrap();
    assert!(
        j2_output.contains("{{"),
        "expected j2 template with placeholders, got: {j2_output}"
    );

    // Step 2: render the template with new data
    let data_json = r#"{"currency_1":"$5,678"}"#;
    let step2 = clipli()
        .args(["convert", "--from", "j2", "--to", "html", "-D", data_json])
        .write_stdin(j2_output)
        .output()
        .unwrap();
    assert!(
        step2.status.success(),
        "step 2 (j2->html) failed: {}",
        String::from_utf8_lossy(&step2.stderr)
    );
    let html_output = String::from_utf8(step2.stdout).unwrap();
    assert!(
        html_output.contains("$5,678"),
        "expected rendered HTML to contain '$5,678', got: {html_output}"
    );
}

// ---------------------------------------------------------------------------
// 2. Table render -> plain text
// ---------------------------------------------------------------------------

#[test]
fn test_table_render_to_plain_text() {
    let table_json = r#"{"headers":[{"value":"Name","style":{"bold":true}},{"value":"Score","style":{"bold":true}}],"rows":[[{"value":"Alice","style":{}},{"value":"95","style":{}}],[{"value":"Bob","style":{}},{"value":"87","style":{}}]]}"#;

    // Step 1: render table to HTML
    let step1 = clipli()
        .args(["paste", "--from-table", "--dry-run"])
        .write_stdin(table_json)
        .output()
        .unwrap();
    assert!(
        step1.status.success(),
        "step 1 (paste table) failed: {}",
        String::from_utf8_lossy(&step1.stderr)
    );
    let html_output = String::from_utf8(step1.stdout).unwrap();

    // Step 2: convert HTML to plain text
    let step2 = clipli()
        .args(["convert", "--from", "html", "--to", "plain"])
        .write_stdin(html_output)
        .output()
        .unwrap();
    assert!(
        step2.status.success(),
        "step 2 (html->plain) failed: {}",
        String::from_utf8_lossy(&step2.stderr)
    );
    let plain_output = String::from_utf8(step2.stdout).unwrap();

    assert!(
        plain_output.contains("Alice"),
        "plain text should contain 'Alice', got: {plain_output}"
    );
    assert!(
        plain_output.contains("Bob"),
        "plain text should contain 'Bob', got: {plain_output}"
    );
    assert!(
        plain_output.contains("95"),
        "plain text should contain '95', got: {plain_output}"
    );
    assert!(
        plain_output.contains("87"),
        "plain text should contain '87', got: {plain_output}"
    );
}

// ---------------------------------------------------------------------------
// 3. Templatize preserves rendering with matching default values
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_preserves_rendering() {
    let original_html = "<table><tr><td>Q1 2024</td><td>$1,000</td><td>10%</td></tr></table>";

    // Step 1: templatize HTML -> J2
    let step1 = clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin(original_html)
        .output()
        .unwrap();
    assert!(
        step1.status.success(),
        "step 1 (html->j2) failed: {}",
        String::from_utf8_lossy(&step1.stderr)
    );
    let j2_output = String::from_utf8(step1.stdout).unwrap();

    // Step 2: render back with default values matching the originals
    let data_json = r#"{"quarter_1":"Q1 2024","currency_1":"$1,000","pct_1":"10%"}"#;
    let step2 = clipli()
        .args(["convert", "--from", "j2", "--to", "html", "-D", data_json])
        .write_stdin(j2_output)
        .output()
        .unwrap();
    assert!(
        step2.status.success(),
        "step 2 (j2->html) failed: {}",
        String::from_utf8_lossy(&step2.stderr)
    );
    let rendered_html = String::from_utf8(step2.stdout).unwrap();

    assert!(
        rendered_html.contains("Q1 2024"),
        "rendered HTML should contain 'Q1 2024', got: {rendered_html}"
    );
    assert!(
        rendered_html.contains("$1,000"),
        "rendered HTML should contain '$1,000', got: {rendered_html}"
    );
    assert!(
        rendered_html.contains("10%"),
        "rendered HTML should contain '10%', got: {rendered_html}"
    );
}

// ---------------------------------------------------------------------------
// 4. Convert chain: realistic HTML table -> plain text
// ---------------------------------------------------------------------------

#[test]
fn test_convert_chain_html_plain() {
    let html = "<table><tr><th>Product</th><th>Price</th></tr><tr><td>Widget</td><td>$9.99</td></tr></table>";

    let output = clipli()
        .args(["convert", "--from", "html", "--to", "plain"])
        .write_stdin(html)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "html->plain failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plain = String::from_utf8(output.stdout).unwrap();

    assert!(
        plain.contains("Product"),
        "plain text should contain 'Product', got: {plain}"
    );
    assert!(
        plain.contains("Price"),
        "plain text should contain 'Price', got: {plain}"
    );
    assert!(
        plain.contains("Widget"),
        "plain text should contain 'Widget', got: {plain}"
    );
    assert!(
        plain.contains("$9.99"),
        "plain text should contain '$9.99', got: {plain}"
    );
}

// ---------------------------------------------------------------------------
// 5. Multiple J2 renders from the same template with different data
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_j2_renders_same_template() {
    let template = "<p>{{ greeting }}, {{ name }}!</p>";

    // Render with first dataset
    let render1 = clipli()
        .args([
            "convert",
            "--from",
            "j2",
            "--to",
            "html",
            "-D",
            r#"{"greeting":"Hello","name":"Alice"}"#,
        ])
        .write_stdin(template)
        .output()
        .unwrap();
    assert!(
        render1.status.success(),
        "render 1 failed: {}",
        String::from_utf8_lossy(&render1.stderr)
    );
    let output1 = String::from_utf8(render1.stdout).unwrap();
    assert!(
        output1.contains("Hello, Alice!"),
        "expected 'Hello, Alice!' in output, got: {output1}"
    );

    // Render with second dataset
    let render2 = clipli()
        .args([
            "convert",
            "--from",
            "j2",
            "--to",
            "html",
            "-D",
            r#"{"greeting":"Hi","name":"Bob"}"#,
        ])
        .write_stdin(template)
        .output()
        .unwrap();
    assert!(
        render2.status.success(),
        "render 2 failed: {}",
        String::from_utf8_lossy(&render2.stderr)
    );
    let output2 = String::from_utf8(render2.stdout).unwrap();
    assert!(
        output2.contains("Hi, Bob!"),
        "expected 'Hi, Bob!' in output, got: {output2}"
    );
}

// ---------------------------------------------------------------------------
// 6. Paste table dry-run produces valid HTML structure
// ---------------------------------------------------------------------------

#[test]
fn test_paste_table_default_produces_valid_html() {
    let table_json = r##"{
  "headers": [{"value":"Name","style":{"bold":true}},{"value":"Score","style":{"bold":true}}],
  "rows": [[{"value":"Alice","style":{}},{"value":"95","style":{}}]],
  "style": {"header_bg":"#1F497D","header_fg":"#FFFFFF"}
}"##;

    let output = clipli()
        .args(["paste", "--from-table", "--dry-run"])
        .write_stdin(table_json)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "paste table failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let html = String::from_utf8(output.stdout).unwrap();

    // Assert valid HTML structure
    let has_doctype = html.contains("<!DOCTYPE") || html.contains("<!doctype");
    let has_html_tag = html.contains("<html") || html.contains("<HTML");
    let has_table_tag = html.contains("<table");
    assert!(
        has_doctype || has_html_tag || has_table_tag,
        "expected valid HTML structure (<!DOCTYPE, <html, or <table), got: {html}"
    );

    // Assert proper closing tags
    assert!(
        html.contains("</table>") || html.contains("</TABLE>"),
        "expected closing </table> tag, got: {html}"
    );
}

// ---------------------------------------------------------------------------
// 7. Empty table renders without crashing
// ---------------------------------------------------------------------------

#[test]
fn test_empty_table_renders() {
    let table_json = r#"{"rows":[[{"value":"","style":{}}]]}"#;

    clipli()
        .args(["paste", "--from-table", "--dry-run"])
        .write_stdin(table_json)
        .assert()
        .success();
}
