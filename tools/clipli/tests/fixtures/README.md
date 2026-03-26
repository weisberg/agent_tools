# Test Fixtures

This directory contains HTML fixture files representing clipboard content copied from Office and productivity applications on macOS. These fixtures are used by the `clean_tests.rs` snapshot tests to verify that the HTML cleaner correctly strips Office cruft and normalizes markup.

---

## Fixtures

### `excel_table_basic.html`

**Source:** Microsoft Excel 16 (macOS)
**Content:** A minimal 3-column, 4-row table with columns: Name, Value, Status.

**Structure:**
- Full `<html>` document with Excel-specific `xmlns` declarations
- `<meta name=Generator content="Microsoft Excel 16">`
- `<style>` block with `.xl65` / `.xl66` class rules containing `mso-*` properties
- `<!--[if gte mso 9]>` conditional comment wrapping an `<x:ExcelWorkbook>` XML island
- `<table>` with `<col>` elements carrying `mso-width-source` and `mso-width-alt` attributes
- Cells with `class=xl65` / `class=xl66` and no inline styles (all styling deferred to the class block)

**Features tested:**
- Stripping `<meta>`, `<link>`, `<style>` blocks
- Stripping `<!--[if gte mso 9]>` conditional comments and embedded XML
- Removing `.xl65` / `.xl66` class attributes
- Removing `mso-width-source` / `mso-width-alt` from `<col>` attributes
- Preserving table structure, `border`, `cellpadding`, `cellspacing` attributes

---

### `excel_table_formatted.html`

**Source:** Microsoft Excel 16 (macOS)
**Content:** A 5-column quarterly financial table (Quarter, Revenue, Expenses, Net Profit, YoY Growth) with conditional color formatting.

**Structure:**
- Excel wrapper identical to `excel_table_basic.html`
- Five CSS classes (`.xl65`–`.xl69`) with varied `background`, `color`, `mso-number-format`, `mso-generic-font-family`, `mso-font-charset`, `mso-pattern` properties
- Header row with dark blue background (`#1F497D`) and white text (`.xl65`)
- Green cells (`background:#C6EFCE`) for positive values (`.xl68`)
- Red cells (`background:#FFC7CE`, `color:#9C0006`) for negative values (`.xl69`)
- Font: Calibri, bold headers
- Frozen-pane XML in the conditional comment block
- `border:.5pt solid windowtext` on all cells

**Features tested:**
- Everything from `excel_table_basic.html`, plus:
- Conversion of `windowtext` → `#000000` in border declarations
- Preservation of explicit `background-color` / `color` values after class removal
- Stripping `mso-number-format`, `mso-generic-font-family`, `mso-font-charset`, `mso-pattern`
- Normalizing `rgb()` color values (if any) to `#RRGGBB`
- Stripping frozen-pane and workbook XML from conditional comments

---

### `ppt_two_slides.html`

**Source:** Microsoft PowerPoint 16 (macOS) — two slides selected and copied
**Content:** Slide 1 is a title slide ("Q4 Business Review"); Slide 2 is a content slide ("Key Highlights") with four bullet points.

**Structure:**
- Full HTML document with PPT-specific `xmlns:p` namespace
- `<meta name=Generator content="Microsoft PowerPoint 16">`
- `<style>` block defining `p.MsoTitle`, `p.MsoNormal`, `p.MsoListBullet` with `mso-*` properties and `+mj-lt` / `+mn-lt` theme font references
- Two `<div class=SlideShowSlide1 id=slide1 ...>` containers with `mso-width-source`, `mso-height-source`, `mso-shapecount` attributes
- `<o:p>` empty inline elements throughout
- `<![if !supportLists]>` non-conditional IE conditionals wrapping bullet symbol spans
- `mso-list:Ignore` spans inside list items

**Features tested:**
- Stripping `<o:p>` elements (empty `<p>` collapse)
- Stripping `<!--[if gte mso 9]>` and `<![if !supportLists]>` / `<![endif]>` conditionals
- Removing `class=SlideShowSlide1`, `class=MsoNormal`, etc.
- Removing `mso-shapecount`, `mso-width-source`, `mso-height-source` from div styles
- Collapsing `mso-list:Ignore` spans
- Font alias normalization: `+mj-lt` → `Calibri`, `+mn-lt` → `Calibri`
- Stripping `mso-fareast-font-family`, `mso-bidi-font-family`, `mso-ansi-language`

---

### `ppt_chart_slide.html`

**Source:** Microsoft PowerPoint 16 (macOS) — a slide containing a bar chart copied to clipboard
**Content:** A single slide titled "Revenue by Region — Q4 2024" with a chart that degrades to a data table.

**Structure:**
- Same PPT wrapper as `ppt_two_slides.html`
- Chart represented as a `<div id=chart1 class=MSO_CH_TextImport>` with `mso-chart-type` and `mso-chart-id` attributes
- Accessible fallback `<table>` with header row (blue `#4472C4` background, white text), data rows, and conditional color cells
- `mso-yfti-irow`, `mso-yfti-firstrow`, `mso-yfti-lastrow` row attributes
- `mso-border-alt` on all cells
- Nested `<p class=MsoNormal>` inside every `<td>`

**Features tested:**
- Stripping `mso-chart-type`, `mso-chart-id` from div attributes
- Stripping `mso-yfti-*` row attributes
- Stripping `mso-border-alt` from cell styles
- Collapsing single-`<p>` wrappers inside `<td>` elements
- Preserving meaningful `background-color` and `color` on header and data cells
- Preserving table borders after `mso-border-alt` removal

---

### `sheets_pivot.html`

**Source:** Google Sheets (web, Chrome on macOS) — pivot table range selected and copied
**Content:** A 4-product × 4-region pivot table showing sales figures with a Grand Total row and column.

**Structure:**
- Standard `<!DOCTYPE html>` with `<meta name="generator" content="Google Sheets">`
- `<style>` block with `.ritz .waffle` rules (Google Sheets class names)
- `<colgroup>` / `<col id="cols-N">` for column width hints
- `<tbody>` only (no `<thead>` / `<tfoot>`)
- `data-row` and `data-col` attributes on every `<th>` and `<td>`
- `class="freezebar-cell freezebar-horizontal-handle"` on the corner cell
- All meaningful styling is inline (`background-color`, `color`, `font-weight`, `text-align`)

**Features tested:**
- Preserving `<colgroup>` / `<col>` structure (or stripping width hints, depending on target)
- Stripping `data-row`, `data-col` custom data attributes
- Stripping `class` attributes (`.ritz`, `.waffle`, `.freezebar-cell`, etc.)
- Handling well-formed HTML5 (contrast with malformed Office HTML)
- Preserving inline styles already in normalized form (no `mso-*` to strip)

---

### `numbers_table.html`

**Source:** Apple Numbers (macOS) — table range selected and copied
**Content:** A 5-row employee table with columns: Employee, Department, Start Date, Salary, Status.

**Structure:**
- `<!DOCTYPE html PUBLIC "-//W3C//DTD HTML 4.01//EN">` (Numbers uses HTML 4.01 strict)
- `<meta name="Generator" content="Cocoa HTML Writer">` and `<meta name="CocoaVersion" ...>`
- `<style>` block with typed class rules (`td.td1`–`td.td9`, `p.p1`–`p.p4`) for alternating row colors and header styling
- `x-apple-data-detectors="false"` non-standard attribute on every `<td>`
- `<p>` wrappers inside every cell (similar to PPT nested paragraph pattern)
- Alternating white (`#ffffff`) and light gray (`#f5f7fa`) row backgrounds
- No `mso-*` properties — Apple-specific patterns only

**Features tested:**
- Stripping `x-apple-data-detectors` non-standard attributes
- Removing `td.td1` / `p.p1` class attributes
- Handling HTML 4.01 strict doctype
- Stripping Apple Cocoa generator meta tags
- Collapsing single-`<p>` wrappers inside `<td>` elements
- No `mso-*` stripping needed — verifies the cleaner doesn't corrupt clean HTML

---

### `word_formatted_text.html`

**Source:** Microsoft Word 16 (macOS) — multi-paragraph document section copied
**Content:** A two-section project status report with a heading, body paragraphs with bold/italic inline formatting, and a four-item bulleted list.

**Structure:**
- Full Word HTML document with `xmlns:w` namespace
- Large `<style>` block defining `p.MsoNormal`, `h1`, `p.MsoListParagraph`, `span.Heading1Char` with extensive `mso-*` properties
- Two `<!--[if gte mso 9]>` blocks: one for `<o:DocumentProperties>`, one for `<w:WordDocument>` settings
- `<!--[if gte mso 10]>` block for `table.MsoNormalTable`
- `<!--[if gte mso 9]>` wrapping a `<w:LatentStyles>` XML island
- `<h1>` headings with `mso-fareast-font-family`, `mso-ansi-language` attributes
- Inline `<b style='mso-bidi-font-weight:normal'>` and `<i style='mso-bidi-font-style:normal'>` spans
- `<span style='color:black;mso-color-alt:windowtext'>` wrapping plain text
- `<o:p>` empty paragraphs used as spacers
- List items with `<![if !supportLists]>` / `<![endif]>` wrapping Symbol-font bullet spans
- `mso-list:Ignore` spans inside each bullet prefix

**Features tested:**
- Stripping all `<!--[if gte mso ...]>` conditional blocks (multiple variants)
- Stripping `<![if !supportLists]>` / `<![endif]>` IE conditionals
- Stripping the full `<style>` block (largest of all fixtures)
- Removing `mso-bidi-font-weight`, `mso-bidi-font-style` from inline spans
- Stripping `mso-color-alt:windowtext` while preserving `color:black`
- Removing `class=MsoNormal`, `class=MsoListParagraph`, `class=Heading1Char`
- Stripping `mso-fareast-font-family`, `mso-ansi-language`, `mso-themecolor`, `mso-themeshade`
- Collapsing `<o:p>` spacer elements
- Collapsing `mso-list:Ignore` bullet-prefix spans
- Preserving `<h1>`, `<b>`, `<i>` semantic markup after class/style stripping

---

## Re-capturing Fixtures from Real Applications

To update or add new fixtures from actual application output, use the `clipli read` command to dump the HTML type from the macOS pasteboard:

```bash
# 1. Select and copy content in the application (Cmd+C)
# 2. Capture the HTML type from the pasteboard:
clipli read --type html > tests/fixtures/<fixture_name>.html

# Or with inspection to verify what types are available:
clipli inspect
clipli read --type html > tests/fixtures/excel_table_basic.html
```

The captured HTML will be the raw bytes that the application placed on the `public.html` (or `WebArchive`) pasteboard type — exactly what the HTML cleaner will process in production.

> Note: The `public.html` type may be UTF-8, UTF-16LE, or Windows-1252 encoded depending on the application version. The cleaner's Decode stage (Stage 1) handles all three. Captured fixtures should be saved as UTF-8 (the shell pipeline above handles this for most apps).

---

## Using Fixtures in Tests

Fixtures are loaded in `tests/clean_tests.rs` using `insta` snapshot testing:

```rust
#[test]
fn test_clean_excel_basic() {
    let html = include_str!("fixtures/excel_table_basic.html");
    let cleaned = clean(html, &CleanOptions::default()).unwrap();
    insta::assert_snapshot!(cleaned);
}
```

Run snapshot tests and update snapshots after intentional changes:

```bash
cargo test --test clean_tests
cargo insta review   # accept or reject snapshot diffs
```
