# table_excel.html.j2 — Excel Clipboard HTML Reference

This document describes everything learned about generating HTML that Excel's paste parser correctly interprets. It serves as the definitive reference for the companion `table_excel.html.j2` template.

---

## Why a separate Excel template exists

Excel's HTML paste parser is completely different from a web browser. Standard CSS-based HTML — `background-color`, `border` shorthand, inline-only styles — is silently ignored. Backgrounds don't render, borders are missing, alignment has no effect, and number formatting is lost.

The `table_excel` template generates HTML that matches what Excel 15/16 itself writes to the clipboard when you copy cells. This was reverse-engineered by copying formatted cells from Excel and reading the clipboard with `clipli read --type html`.

---

## Anatomy of Excel clipboard HTML

### Document envelope

```html
<html xmlns:v="urn:schemas-microsoft-com:vml"
xmlns:o="urn:schemas-microsoft-com:office:office"
xmlns:x="urn:schemas-microsoft-com:office:excel"
xmlns="http://www.w3.org/TR/REC-html40">
<head>
<meta http-equiv=Content-Type content="text/html; charset=utf-8">
<meta name=ProgId content=Excel.Sheet>
<meta name=Generator content="Microsoft Excel 15">
```

- The **Office XML namespaces** (`xmlns:x`, `xmlns:o`) are required.
- **`ProgId=Excel.Sheet`** tells Excel "this is native Excel HTML" — without it, Excel treats the content as generic web HTML and ignores most formatting.
- The `Generator` tag is cosmetic but Excel does emit it.

### Style block structure

Excel reads formatting from a `<style>` block wrapped in an HTML comment (`<!-- ... -->`). This is not optional — inline styles alone do not reliably control backgrounds or borders.

```html
<style>
<!--table
    {mso-displayed-decimal-separator:"\.";
    mso-displayed-thousand-separator:"\,";}
td
    {padding-top:1px;
    padding-right:1px;
    padding-left:1px;
    mso-ignore:padding;
    color:black;
    font-size:11.0pt;
    font-weight:400;
    font-style:normal;
    text-decoration:none;
    font-family:"Calibri", sans-serif;
    mso-font-charset:0;
    mso-number-format:General;
    text-align:general;
    vertical-align:bottom;
    border:none;
    mso-background-source:auto;
    mso-pattern:auto;
    mso-protection:locked visible;
    white-space:nowrap;
    mso-rotate:0;}
.xl65
    {color:#FFFFFF;
    font-weight:700;
    border-top:1.0pt solid windowtext;
    border-right:none;
    border-bottom:.5pt solid windowtext;
    border-left:1.0pt solid windowtext;
    background:#007873;
    mso-pattern:black none;}
-->
</style>
```

Key rules:

| Property | Correct | Wrong (ignored by Excel) |
|----------|---------|--------------------------|
| Background fill | `background:#007873` | `background-color:#007873` |
| Background companion | `mso-pattern:black none` | `mso-pattern:auto` (only for cells WITHOUT background) |
| Font weight | `font-weight:700` | `font-weight:bold` |
| Border | `border-top:1.0pt solid windowtext` | `border:1px solid #333` |
| Number format | `mso-number-format:General` | (omitting it entirely) |

### Base `td` style

The base `td` selector in the `<style>` block defines the default for every cell. All properties must be present — Excel does not inherit from browser defaults. The base `td` uses:
- `border:none` — individual cell classes add their own borders
- `mso-pattern:auto` — default for cells without a background fill
- `mso-number-format:General` — default number format
- `text-align:general` — Excel's auto-alignment (numbers right, text left)

---

## Border system

Excel uses individual `border-top`, `border-right`, `border-bottom`, `border-left` properties. Each cell position in the table needs a different border combination to create the visual effect of a thick outer frame with thin inner gridlines.

### Border weights
- **Thick (outer frame)**: `1.0pt solid windowtext`
- **Thin (inner gridlines)**: `.5pt solid windowtext`
- **None**: `none`

### Cell position classes

The template defines 9 classes for the 3x3 grid of positions:

**Header row:**
| Position | Class | Top | Right | Bottom | Left |
|----------|-------|-----|-------|--------|------|
| First column | `hdr_l` | 1.0pt | none | .5pt | 1.0pt |
| Middle columns | `hdr_m` | 1.0pt | none | .5pt | none |
| Last column | `hdr_r` | 1.0pt | 1.0pt | .5pt | none |

**Data rows:**
| Position | Class | Top | Right | Bottom | Left |
|----------|-------|-----|-------|--------|------|
| First column | `cl` | .5pt | none | .5pt | 1.0pt |
| Middle columns | `cm` | .5pt | none | .5pt | none |
| Last column | `cr` | .5pt | 1.0pt | .5pt | none |

**Total row (last row with background):**
| Position | Class | Top | Right | Bottom | Left |
|----------|-------|-----|-------|--------|------|
| First column | `tl` | .5pt | none | 1.0pt | 1.0pt |
| Middle columns | `tm` | .5pt | none | 1.0pt | none |
| Last column | `tr` | .5pt | 1.0pt | 1.0pt | none |

### Preventing doubled borders

Because `border-collapse:collapse` is set on the table, adjacent cell borders merge. Data rows add `border-top:none` in their inline `style=` attribute to prevent the top border from doubling with the previous row's bottom border:

```html
<td class=cm style='border-top:none;'>$4,230,000</td>
```

---

## Number formatting

Excel number formats are set via the `mso-number-format` CSS property. Without this, Excel treats pasted values as plain text — currency and percentage values won't be recognized as numbers.

| Format | Property value | Cell displays |
|--------|---------------|---------------|
| Currency | `mso-number-format:"\0022$\0022\#\,\#\#0_\)\;\[Red\]\\\(\0022$\0022\#\,\#\#0\\\)"` | `$4,230,000` with red negatives |
| Percent | `mso-number-format:Percent` | `15.60%` |
| General | `mso-number-format:General` | Auto-detect (set in base `td`) |

The currency format string is complex because it encodes:
- `\0022` = literal `"` character (Unicode escape inside CSS)
- `$` sign prefix
- `\#\,\#\#0` = number with comma grouping, no decimals
- `_\)` = space padding for alignment
- `\;\[Red\]` = semicolon separator, then red format for negatives
- `\\\(` / `\\\)` = parentheses around negatives

Number format is set as an inline style override since different cells in the same column can have different formats:
```html
<td class=cm style='border-top:none;text-align:right;mso-number-format:Percent;'>15.60%</td>
```

---

## Alignment

Alignment in Excel HTML requires **both** approaches simultaneously:

1. **HTML `align=` attribute** on the `<td>` element
2. **`text-align:` in the inline `style=`** attribute

The HTML attribute alone is not sufficient because the class-level `text-align:general` takes precedence in Excel's CSS cascade. The inline style overrides the class.

```html
<!-- WRONG — alignment ignored by Excel -->
<td class=cm align=center>On Track</td>

<!-- CORRECT — both attribute and inline style -->
<td class=cm style='border-top:none;text-align:center;' align=center>On Track</td>
```

The header cells follow the same pattern — alignment must be in both the `align=` attribute and an inline `style='text-align:...'`.

---

## Font handling

The font family is set in the base `td` style in the `<style>` block. Font names with spaces must be quoted:

```css
font-family:"Aptos Display", sans-serif;
mso-font-charset:0;
```

The `mso-font-charset:0` property specifies the ANSI character set and should always be present.

In the template, the font comes from `style.default_font` in the JSON input (NOT a top-level `default_font` key — see implementation notes below).

---

## Table structure

```html
<table border=0 cellpadding=0 cellspacing=0 style='border-collapse:collapse'>
<!--StartFragment-->
 <tr height=21 style='height:16.0pt'>
  <td class=hdr_l>Region</td>
  ...
 </tr>
 <tr height=21 style='height:16.0pt'>
  <td class=cl style='border-top:none;'>North America</td>
  ...
 </tr>
<!--EndFragment-->
</table>
```

- `border=0` on the table — all borders come from CSS classes, not the table attribute
- `<!--StartFragment-->` and `<!--EndFragment-->` markers bracket the cell data
- Each `<tr>` has `height=21 style='height:16.0pt'` (Excel's default row height)
- Column widths can optionally be set with `<col>` elements

---

## Template usage (clipli)

```bash
echo '<JSON>' | clipli paste --from-table -t table_excel
```

### Style object

All table-level configuration goes inside the `style` object in the JSON input:

| Key | Default | Description |
|-----|---------|-------------|
| `style.header_bg` | `#4472C4` | Header row background color |
| `style.header_fg` | `#FFFFFF` | Header row text color |
| `style.total_bg` | `#F2F2F2` | Total (last) row background |
| `style.default_font` | `Calibri` | Font family |
| `style.default_font_size` | `11` | Font size in pt |

### Per-cell style

Each cell has a `style` object with these fields:

| Field | Type | Description |
|-------|------|-------------|
| `alignment` | `"left"`, `"center"`, `"right"` | Text alignment |
| `bold` | boolean | Bold text |
| `fg_color` | hex string (e.g. `"#2E7D32"`) | Text color |
| `bg_color` | hex string | Background (also triggers total-row class on last row) |
| `number_format` | `"currency"`, `"percent"` | Excel number format; omit for General |

### Complete example

```json
{
  "headers": [
    {"value": "Region", "style": {}},
    {"value": "Revenue", "style": {"alignment": "right"}},
    {"value": "Growth", "style": {"alignment": "center"}}
  ],
  "rows": [
    [
      {"value": "North America", "style": {"bold": true}},
      {"value": "$4,230,000", "style": {"number_format": "currency", "alignment": "right"}},
      {"value": "15.60%", "style": {"number_format": "percent", "alignment": "center", "fg_color": "#2E7D32"}}
    ],
    [
      {"value": "Total", "style": {"bold": true, "bg_color": "#F2F2F2"}},
      {"value": "$4,230,000", "style": {"number_format": "currency", "alignment": "right", "bold": true, "bg_color": "#F2F2F2"}},
      {"value": "15.60%", "style": {"number_format": "percent", "alignment": "center", "bold": true, "fg_color": "#2E7D32", "bg_color": "#F2F2F2"}}
    ]
  ],
  "style": {
    "header_bg": "#007873",
    "header_fg": "#FFFFFF",
    "total_bg": "#F2F2F2",
    "default_font": "Aptos Display"
  }
}
```

---

## Implementation pitfalls

### 1. serde `Option::None` serializes as JSON `null`, which breaks minijinja `| default()`

Rust `Option::None` becomes JSON `null`. In minijinja, `null` is a *defined* value, so `{{ field | default('fallback') }}` does NOT trigger — it renders the literal string `"none"`. This caused `background-color:none` and `font-size:nonept`.

**Fix:** Add `#[serde(skip_serializing_if = "Option::is_none")]` to all `Option<T>` fields in the model structs (`CellStyle`, `TableStyle`, `BorderStyle`). This omits the field entirely from the JSON when `None`, making minijinja see it as undefined so `| default()` works.

### 2. Top-level JSON keys outside `TableInput` are silently dropped

The `TableInput` struct has `headers`, `rows`, and `style`. Any other top-level key (like `default_font`) is silently discarded during serde deserialization. Template variables like font and size must live inside the `style` object.

### 3. Font names with spaces need CSS quotes

`font-family:Aptos Display, sans-serif` is invalid CSS — the browser (and Excel) will only see `Aptos` as the font name. Must be: `font-family:"Aptos Display", sans-serif`.

### 4. `text-align` must be in BOTH HTML attribute and inline style

Excel's CSS cascade gives the class-level `text-align:general` higher priority than the `align=` HTML attribute. The inline `style='text-align:center'` is the only way to override it. Both are needed for maximum compatibility.

---

## Reference: actual Excel 15 clipboard output

This is what Excel 15 (macOS) puts on the clipboard when you copy formatted cells. The template was designed to match this structure exactly.

Key observations from the real Excel output:
- Every unique combination of formatting gets its own numbered class (`.xl65`, `.xl66`, etc.)
- The base `td` rule defines ALL default properties — nothing is left to browser defaults
- `mso-pattern:black none` is used for cells WITH a background fill
- `mso-pattern:auto` is used for cells WITHOUT a background fill (set in base `td`)
- `windowtext` is the keyword for the default text/border color (resolves to black)
- Row heights and column widths are explicit (`height:16.0pt`, `width:75pt`)
- `mso-width-source:userset` indicates user-set column widths
- `<col>` elements define column widths before the first `<tr>`
- `<!--[if gte mso 9]>` conditional comment blocks contain XML workbook metadata (optional)

To capture a fresh reference from Excel at any time:
```bash
# Copy cells in Excel, then:
clipli read --type html > excel_reference.html
```

---

## Column widths — NOT supported on paste

**Confirmed 2026-03-26:** Column widths specified via `<col>` elements are completely ignored when pasting clipboard HTML into Excel. Tested with `mso-width-source:userset`, `mso-width-alt`, pixel `width` attributes on `<col>`, and matching `width`/`style` on header `<td>` elements — none had any effect.

Excel auto-fits columns on paste regardless. Column widths are only respected when opening an HTML file as a workbook (File > Open), not on clipboard paste.

Workarounds:
- Auto-fit columns after paste (select columns, double-click border)
- Save HTML to a `.html` file and open in Excel instead of pasting
- Use AppleScript/VBA to resize columns programmatically after paste

---

## Merged title rows (colspan)

Excel supports `colspan` for merged cells. A title row spanning the full table width uses:

```html
<td colspan=7 height=35 class=xl74 width=609
    style='border-right:1.0pt solid black;height:26.0pt;width:455pt'>April 2026</td>
```

Key details:
- The merged cell needs `border-right` as an **inline style override** because the class only defines the left border (the right edge of the merge is the table's right edge)
- Excel defines separate classes for left/middle/right positions of merged cells (`xl74/xl75/xl76`) even though only one `<td>` is emitted — only the leftmost class is used on the actual element
- For date-formatted title rows, use `mso-number-format:"mmmm\\ yyyy"` to tell Excel this is a date displayed as "April 2026"
- The header row below the title needs `border-top:none` to avoid a doubled border (the title's `border-bottom:1.0pt solid windowtext` handles it)

### Font charset for Aptos

When using Aptos Display or Aptos Narrow, Excel uses `mso-font-charset:1` (DEFAULT_CHARSET) rather than `mso-font-charset:0` (ANSI_CHARSET) which is used for Calibri. The charset value tells Excel which character encoding the font supports.

### Empty cells

Excel writes `&nbsp;` in empty cells, not an empty string. This ensures the cell occupies space in the rendered table.

### Data cell borders

In Excel's actual output, data cells (non-header, non-total) do NOT use `border-top:none` in their inline styles — the class-level `border-top:none` is already set in the class definition itself. Only the header row uses an inline `border-top:none` override (to cancel the class's `border-top:1.0pt` when the title row above provides the border).

### Base font

The base `td` style uses `font-family:"Aptos Narrow", sans-serif` as the default font (Excel 15/365 default). Individual classes override with `font-family:"Aptos Display", sans-serif` when a different font is applied to those cells.
