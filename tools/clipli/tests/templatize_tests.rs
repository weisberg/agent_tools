use assert_cmd::Command;
use predicates::prelude::*;

fn clipli() -> Command {
    Command::cargo_bin("clipli").unwrap()
}

// ---------------------------------------------------------------------------
// 1. Currency detection — dollar amount replaced with {{ currency_1 }}
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_currency_detection() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>$4,200,000</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ currency_1 }}"))
        .stdout(predicate::str::contains("$4,200,000").not());
}

// ---------------------------------------------------------------------------
// 2. Multiple currencies — each gets a unique numbered variable
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_multiple_currencies() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>$100</td><td>$200</td><td>$300</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("currency_1"))
        .stdout(predicate::str::contains("currency_2"))
        .stdout(predicate::str::contains("currency_3"));
}

// ---------------------------------------------------------------------------
// 3. ISO date — YYYY-MM-DD replaced with {{ date_1 }}
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_iso_date() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>2024-03-15</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ date_1 }}"));
}

// ---------------------------------------------------------------------------
// 4. US date — MM/DD/YYYY replaced with {{ date_1 }}
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_us_date() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>03/15/2024</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ date_1 }}"));
}

// ---------------------------------------------------------------------------
// 5. Written date — "March 15, 2024" replaced with {{ date_1 }}
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_written_date() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<p>March 15, 2024</p>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ date_1 }}"));
}

// ---------------------------------------------------------------------------
// 6. Percentage — "12.5%" replaced with {{ pct_1 }}
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_percentage() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>12.5%</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ pct_1 }}"));
}

// ---------------------------------------------------------------------------
// 7. Email — address replaced with {{ email_1 }}
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_email() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>user@example.com</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ email_1 }}"));
}

// ---------------------------------------------------------------------------
// 8. Large number — comma-formatted number replaced with {{ number_1 }}
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_large_number() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>1,234,567</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ number_1 }}"));
}

// ---------------------------------------------------------------------------
// 9. Quarter — "Q3 2024" replaced with {{ quarter_1 }}
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_quarter() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>Q3 2024</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ quarter_1 }}"));
}

// ---------------------------------------------------------------------------
// 10. Mixed types — currency, date, and percentage all detected together
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_mixed_types() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>$5,000</td><td>2024-01-01</td><td>15%</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("currency_1"))
        .stdout(predicate::str::contains("date_1"))
        .stdout(predicate::str::contains("pct_1"));
}

// ---------------------------------------------------------------------------
// 11. HTML structure preserved — tags survive templatization
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_preserves_html_structure() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<table><tr><td>$100</td></tr></table>")
        .assert()
        .success()
        .stdout(predicate::str::contains("<table>"))
        .stdout(predicate::str::contains("<tr>"))
        .stdout(predicate::str::contains("<td>"))
        .stdout(predicate::str::contains("</td>"))
        .stdout(predicate::str::contains("</tr>"))
        .stdout(predicate::str::contains("</table>"));
}

// ---------------------------------------------------------------------------
// 12. Attributes not replaced — href containing "$100" is preserved
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_no_replacement_in_attributes() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin(r#"<a href="$100.html">$100</a>"#)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"href="$100.html""#));
}

// ---------------------------------------------------------------------------
// 13. Structural labels skipped — "Total" stays literal
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_structural_labels_skipped() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>Total</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("Total"));
}

// ---------------------------------------------------------------------------
// 14. Cell field detection — non-pattern text in <td> becomes {{ field_N }}
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_cell_field_detection() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<table><tr><td>Acme Corporation</td></tr></table>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ field_1 }}"));
}

// ---------------------------------------------------------------------------
// 15. Empty input — succeeds with empty or minimal output
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_empty_input() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("")
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// 16. No dynamic content — plain text in <p> passes through unchanged
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_no_dynamic_content() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<p>Hello World</p>")
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello World"));
}

// ---------------------------------------------------------------------------
// 17. Realistic Excel table — currency, percentage, and field detection
// ---------------------------------------------------------------------------

#[test]
fn test_templatize_realistic_excel_table() {
    let html = concat!(
        "<table>",
        "<tr><th>Product</th><th>Revenue</th><th>Growth</th></tr>",
        "<tr><td>Widget A</td><td>$1,200,000</td><td>15.3%</td></tr>",
        "<tr><td>Widget B</td><td>$800,000</td><td>-2.1%</td></tr>",
        "</table>"
    );

    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin(html)
        .assert()
        .success()
        .stdout(predicate::str::contains("currency_1"))
        .stdout(predicate::str::contains("pct_1"))
        .stdout(predicate::str::contains("field_1").or(predicate::str::contains("field_2")));
}
