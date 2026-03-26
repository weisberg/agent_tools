# Excel Clipboard HTML Specification

## Definitive reference for generating HTML that Excel's paste parser correctly interprets

**Version:** 2.0 — Expanded from the `table_excel.html.j2` reverse-engineering notes  
**Scope:** Covers HTML placed on the system clipboard (via CF_HTML on Windows, `public.html` / NSPasteboard on macOS) that, when pasted into Excel 15/16/365, preserves formatting including backgrounds, borders, fonts, number formats, and alignment.

---

## 1. Context: How Clipboard HTML Reaches Excel

### 1.1 Clipboard Formats Excel Reads

When cells are pasted into Excel, the application inspects the clipboard for formats in a priority order. Excel prefers its own native formats (Biff8, XML Spreadsheet) over HTML. When only HTML is available — for example, when pasting from a browser or a non-Excel application — Excel reads the `HTML Format` (CF_HTML) clipboard type on Windows, or the `public.html` pasteboard type on macOS.

Excel also places many formats on the clipboard simultaneously when you copy cells *out*: Biff8, Biff5, XML Spreadsheet, HTML Format, CF_UNICODETEXT, Rich Text Format, CSV, SYLK, DIF, and others. The HTML format is just one of roughly 20 clipboard entries Excel writes.

### 1.2 CF_HTML Envelope (Windows)

On Windows, the `HTML Format` clipboard data is not raw HTML. It is wrapped in a text envelope with byte-offset headers:

```
Version:0.9
StartHTML:0000000105
EndHTML:0000002347
StartFragment:0000000141
EndFragment:0000002311
<html>
<head>...</head>
<body>
<!--StartFragment-->
  ...table content...
<!--EndFragment-->
</body>
</html>
```

The `Version`, `StartHTML`, `EndHTML`, `StartFragment`, and `EndFragment` fields are mandatory. Offset values are byte offsets (in UTF-8) from the beginning of the entire string, not character offsets. Programs often pre-allocate 10-digit zero-padded fields and overwrite from the right once offsets are known.

The `<!--StartFragment-->` and `<!--EndFragment-->` comment markers bracket the actual content Excel will import. Everything outside these markers is context (parent elements, style blocks) that Excel uses for formatting resolution but does not import as cell data.

### 1.3 macOS Pasteboard

On macOS, Excel reads from the `public.html` pasteboard type (NSPasteboard). The HTML is stored as raw UTF-8 without a CF_HTML-style byte-offset envelope. The `<!--StartFragment-->` / `<!--EndFragment-->` markers are still recognized and respected by Excel for Mac, and should be included for compatibility even though the offset header is absent.

### 1.4 Browser `text/html` MIME Type

When writing to the clipboard from JavaScript (via `e.clipboardData.setData('text/html', str)` or the async Clipboard API), the browser maps the `text/html` type to the platform-native HTML clipboard format. On Windows this becomes CF_HTML with the browser generating the envelope; on macOS it becomes `public.html`. The HTML content itself is what you control.

---

## 2. Document Envelope and Meta Tags

### 2.1 Required Structure

```html
<html xmlns:v="urn:schemas-microsoft-com:vml"
xmlns:o="urn:schemas-microsoft-com:office:office"
xmlns:x="urn:schemas-microsoft-com:office:excel"
xmlns="http://www.w3.org/TR/REC-html40">
<head>
<meta http-equiv=Content-Type content="text/html; charset=utf-8">
<meta name=ProgId content=Excel.Sheet>
<meta name=Generator content="Microsoft Excel 15">
<style>
<!--
  ...style definitions...
-->
</style>
</head>
<body>
<table border=0 cellpadding=0 cellspacing=0 style='border-collapse:collapse'>
<!--StartFragment-->
  ...rows...
<!--EndFragment-->
</table>
</body>
</html>
```

### 2.2 Why Each Element Matters

| Element | Purpose | Consequence If Missing |
|---------|---------|----------------------|
| `xmlns:x="urn:schemas-microsoft-com:office:excel"` | Enables the `x:` namespace for data type attributes (`x:num`, `x:str`, `x:bool`, `x:fmla`) | Cell data type hints are silently ignored |
| `xmlns:o="urn:schemas-microsoft-com:office:office"` | Enables Office-specific features | Conditional comment blocks (`<!--[if gte mso 9]>`) ignored |
| `xmlns:v="urn:schemas-microsoft-com:vml"` | VML graphics support (shapes, images) | VML content not rendered |
| `<meta name=ProgId content=Excel.Sheet>` | Tells Excel this is native Excel HTML, not generic web HTML | Excel treats content as browser HTML and ignores most `mso-*` properties and Office CSS |
| `<meta name=Generator content="Microsoft Excel 15">` | Cosmetic; Excel emits it when copying | No functional impact observed, but recommended for fidelity |
| `charset=utf-8` | Character encoding declaration | Accented and non-Latin characters may be mangled |

**Critical:** Without the `ProgId=Excel.Sheet` meta tag, Excel's paste parser falls back to a much more limited "web HTML" import path where backgrounds, `mso-*` properties, and many border styles are silently discarded.

### 2.3 Optional: Conditional XML Workbook Metadata

Excel's own clipboard output sometimes includes conditional comment blocks with XML workbook metadata:

```html
<!--[if gte mso 9]><xml>
 <x:ExcelWorkbook>
  <x:ExcelWorksheets>
   <x:ExcelWorksheet>
    <x:Name>Sheet1</x:Name>
    <x:WorksheetOptions>
     <x:Selected/>
     <x:Panes>...</x:Panes>
    </x:WorksheetOptions>
   </x:ExcelWorksheet>
  </x:ExcelWorksheets>
  <x:WindowHeight>12000</x:WindowHeight>
  <x:WindowWidth>18000</x:WindowWidth>
 </x:ExcelWorkbook>
</xml><![endif]-->
```

This block is **optional** for clipboard paste. It provides worksheet naming, pane/freeze configuration, and window sizing hints. When absent, Excel applies defaults. When generating clipboard HTML for paste, this block can be omitted without affecting cell data or formatting.

---

## 3. Style Block Architecture

### 3.1 HTML Comment Wrapping

Excel's CSS parser reads the style block **only** when it is wrapped in an HTML comment inside the `<style>` element:

```html
<style>
<!--
  ...style definitions...
-->
</style>
```

This is not optional. Inline styles alone do not reliably control backgrounds or borders in Excel's paste parser.

### 3.2 Base `td` Style — The Universal Default

Every property that a cell might need must be explicitly declared in the base `td` rule. Excel does not inherit from browser defaults. Omitting a property means Excel uses an internal fallback that may differ from what you expect.

```css
table
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
```

### 3.3 Property-by-Property Reference for Base `td`

| Property | Value | Purpose |
|----------|-------|---------|
| `padding-top/right/left:1px` | Default cell padding | Must match Excel's internal default |
| `mso-ignore:padding` | Tells Excel to use its own padding rules | Without this, padding may be doubled |
| `color:black` | Default font color | Use `windowtext` for "auto" color |
| `font-size:11.0pt` | Default font size | Excel's default since Excel 2007 |
| `font-weight:400` | Normal weight | Use `700` for bold (not `bold`) |
| `font-style:normal` | Not italic | |
| `text-decoration:none` | No underline/strikethrough | |
| `font-family:"Calibri", sans-serif` | Default font | Names with spaces **must** be quoted |
| `mso-font-charset:0` | ANSI character set for Calibri | Use `1` (DEFAULT_CHARSET) for Aptos family |
| `mso-number-format:General` | Auto-detect number type | See §7 for all format options |
| `text-align:general` | Numbers right, text left (auto) | See §6 for alignment details |
| `vertical-align:bottom` | Excel default | |
| `border:none` | No borders by default | Individual classes add borders |
| `mso-background-source:auto` | No explicit background | |
| `mso-pattern:auto` | Default for cells WITHOUT background | Use `black none` for cells WITH background |
| `mso-protection:locked visible` | Standard cell protection | |
| `white-space:nowrap` | No word wrap | Use `normal` to enable wrapping |
| `mso-rotate:0` | No text rotation | |

### 3.4 Table-Level Separator Properties

The `table` rule's `mso-displayed-decimal-separator` and `mso-displayed-thousand-separator` properties tell Excel which characters appear in the formatted cell text for localization. These affect how Excel parses numbers from the HTML content. For US English:

```css
table
    {mso-displayed-decimal-separator:"\.";
    mso-displayed-thousand-separator:"\,";}
```

For European locales where commas are decimal separators and periods are thousand separators, swap them. These properties apply to cells that do **not** use the `x:num` attribute (see §8).

### 3.5 Additional Table-Level Properties

| Property | Purpose |
|----------|---------|
| `mso-displayed-decimal-separator` | Decimal separator character in displayed cell values |
| `mso-displayed-thousand-separator` | Thousands separator character in displayed cell values |

Additional `tr` and `col` level properties Excel emits:

```css
tr
    {mso-height-source:auto;}
col
    {mso-width-source:auto;}
```

The `mso-height-source` and `mso-width-source` properties indicate whether row height / column width is auto-calculated or user-set. When set to `userset`, the explicit dimensions are respected.

### 3.6 Named Cell Classes

Excel assigns a unique numbered class (`.xl65`, `.xl66`, etc.) to every unique combination of formatting. The convention for class names:

- Excel's built-in style names start with `style` (e.g., `style0` for Normal)
- Cell formatting classes start with `xl` followed by a sequential number
- Rich text fragment classes use `font` prefixes

For the template system described in this spec, descriptive names (`hdr_l`, `cm`, `tl`) are used instead of sequential numbers for clarity.

---

## 4. Background Fills and Patterns

### 4.1 Solid Background Fills

Applying a background color to a cell requires **two** properties working together:

```css
.xl65
    {background:#007873;
    mso-pattern:black none;}
```

| Property | Correct Value | Wrong Value (Ignored) |
|----------|--------------|----------------------|
| Fill color | `background:#007873` | `background-color:#007873` |
| Pattern companion | `mso-pattern:black none` | (omitting it or leaving `auto`) |

**Critical:** Using `background-color` instead of `background` is silently ignored. The `background` shorthand property is what Excel's parser recognizes.

**Critical:** When a cell HAS a background fill, `mso-pattern` must be set to `black none` (meaning "no pattern, solid fill"). When a cell does NOT have a background, `mso-pattern:auto` is the correct default (set in the base `td` rule). Getting this wrong causes fills to be invisible.

### 4.2 Pattern Fills

Excel supports patterned fills beyond solid colors. The `mso-pattern` property takes two values: the pattern name and the pattern foreground color.

```css
mso-pattern:gray-50 #FF0000;
```

Available pattern names include: `none`, `gray-50` (50% gray), `gray-75`, `gray-25`, `gray-125` (12.5%), `gray-0625` (6.25%), `horiz-stripe`, `vert-stripe`, `reverse-diag-stripe`, `diag-stripe`, `diag-crosshatch`, `thick-diag-crosshatch`, `thin-horiz-stripe`, `thin-vert-stripe`, `thin-reverse-diag-stripe`, `thin-diag-stripe`, `thin-horiz-crosshatch`, `thin-diag-crosshatch`.

For patterned fills, the background color goes in the `background` property and the pattern foreground color goes in the `mso-pattern` property's color value.

### 4.3 Background on `<tr>` Elements

Background colors set on `<tr>` elements via class styles are **not** always inherited by the `<td>` elements within that row. For reliable results, always apply background styles directly to `<td>` classes.

---

## 5. Border System

### 5.1 Border Property Syntax

Excel uses individual directional border properties. The shorthand `border: 1px solid #333` is silently ignored.

```css
border-top:1.0pt solid windowtext;
border-right:none;
border-bottom:.5pt solid windowtext;
border-left:1.0pt solid windowtext;
```

Each border property takes three values: **weight**, **style**, and **color**.

### 5.2 Border Weights

| Visual | Weight | CSS Declaration |
|--------|--------|----------------|
| None | 0 | `border-{side}:none` |
| Thin (hairline-ish) | 0.5pt | `border-{side}:.5pt solid color` |
| Medium | 1.0pt | `border-{side}:1.0pt solid color` |
| Thick | 1.5pt | `border-{side}:1.5pt solid color` |
| Double | 1.5pt | `border-{side}:1.5pt double color` |

Excel maps these CSS values to its internal border styles:

| CSS Weight | CSS Style | Excel Result |
|------------|-----------|-------------|
| < 1pt | solid | Thin |
| 1pt to < 1.5pt | solid | Medium |
| >= 1.5pt | solid | Thick |
| < 1pt | dashed | Dashed |
| >= 1pt | dashed | Medium dashed |
| any | dotted | Dotted |
| any | double | Double |
| any | hairline | Hairline |

### 5.3 Border Styles (Extended)

Beyond `solid`, Excel recognizes these CSS border-style values:

| Border Style | CSS | Notes |
|-------------|-----|-------|
| Hairline | `.5pt solid color; border-style:hairline` | Must combine shorthand + override |
| Dash-dot | `.5pt dashed color; border-style:dash-dot` | |
| Medium dash-dot | `1pt dashed color; border-style:dash-dot` | |
| Dash-dot-dot | `.5pt dashed color; border-style:dash-dot-dot` | |
| Medium dash-dot-dot | `1pt dashed color; border-style:dash-dot-dot` | |
| Slanted dash-dot | `1pt dashed color; border-style:slanted-dash-dot` | |

### 5.4 Diagonal Borders

Excel supports diagonal borders using `mso-` prefixed properties:

| Direction | Property |
|-----------|----------|
| Top-left to bottom-right | `mso-border-down` |
| Bottom-left to top-right | `mso-border-up` |

These are not supported by the Spreadsheet Component.

### 5.5 Border Colors

The keyword `windowtext` resolves to the system's default text color (black in most themes) and is how Excel represents "auto" border color. Hex RGB values (`#000000`) also work.

### 5.6 Preventing Doubled Borders with `border-collapse:collapse`

When `border-collapse:collapse` is set on the table, adjacent cell borders merge. This creates a problem: two cells sharing an edge both declare a border, and the resulting border appears doubled or has the wrong weight.

The standard technique is to use inline `style='border-top:none;'` overrides on data cells to suppress their top border (since the row above already provides a bottom border for that edge):

```html
<td class=cm style='border-top:none;'>$4,230,000</td>
```

**Observation from Excel's actual output:** In Excel 15's real clipboard HTML, data cells (non-header, non-total) do NOT always use `border-top:none` as an inline override — some class definitions already set `border-top:none` within the class itself. Only header rows use an inline `border-top:none` override when a title row above already provides the dividing border.

### 5.7 Cell Position Classes (9-Cell Border Grid)

For a table with a thick outer frame and thin inner gridlines, 9 classes cover all positions:

**Header row:**

| Position | Class | Top | Right | Bottom | Left |
|----------|-------|-----|-------|--------|------|
| First col | `hdr_l` | 1.0pt solid | none | .5pt solid | 1.0pt solid |
| Middle cols | `hdr_m` | 1.0pt solid | none | .5pt solid | none |
| Last col | `hdr_r` | 1.0pt solid | 1.0pt solid | .5pt solid | none |

**Data rows:**

| Position | Class | Top | Right | Bottom | Left |
|----------|-------|-----|-------|--------|------|
| First col | `cl` | .5pt solid | none | .5pt solid | 1.0pt solid |
| Middle cols | `cm` | .5pt solid | none | .5pt solid | none |
| Last col | `cr` | .5pt solid | 1.0pt solid | .5pt solid | none |

**Total/last row:**

| Position | Class | Top | Right | Bottom | Left |
|----------|-------|-----|-------|--------|------|
| First col | `tl` | .5pt solid | none | 1.0pt solid | 1.0pt solid |
| Middle cols | `tm` | .5pt solid | none | 1.0pt solid | none |
| Last col | `tr` | .5pt solid | 1.0pt solid | 1.0pt solid | none |

---

## 6. Alignment

### 6.1 Dual-Declaration Requirement

Alignment in Excel HTML requires **both** approaches simultaneously:

1. The `align=` HTML attribute on `<td>`
2. `text-align:` in the inline `style=` attribute

The HTML attribute alone is overridden by the class-level `text-align:general`. The inline style is the only way to override it. Both are needed for maximum cross-version compatibility.

```html
<!-- WRONG — alignment ignored -->
<td class=cm align=center>On Track</td>

<!-- CORRECT — both attribute and inline style -->
<td class=cm style='border-top:none;text-align:center;' align=center>On Track</td>
```

### 6.2 Horizontal Alignment Values

| Alignment | `text-align` Value | HTML `align=` Attribute | Notes |
|-----------|-------------------|------------------------|-------|
| General (auto) | `general` | (omit or `right` for numbers) | Numbers right-align, text left-aligns |
| Left | `left` | (omit — default for browsers) | |
| Left with indent | `left; margin-left: Nem` | (omit) | N is indent in em units |
| Center | `center` | `center` | |
| Right | `right` | `right` | |
| Fill | `fill` | (none) | Repeats content to fill cell width |
| Justify | `justify` | (none) | |
| Distributed | `distributed` | (none) | International |
| Center across selection | `center` + `colspan=N` | `center` | Uses `mso-ignore:colspan` |

### 6.3 Vertical Alignment Values

| Alignment | `vertical-align` | HTML `valign=` |
|-----------|-----------------|----------------|
| Top | `top` | `top` |
| Center | `middle` | `middle` |
| Bottom (default) | `bottom` | `bottom` |
| Justify | `justify` | (none) |
| Distributed | `distributed` | (none) |

### 6.4 Text Wrapping

```css
/* Word wrap enabled */
white-space:normal;

/* Word wrap disabled (default) */
white-space:nowrap;
```

When HTML is opened in Excel, word wrap is enabled for all imported cells. Documents created in Excel have it disabled by default. The `white-space:normal` value in CSS is what triggers Excel to enable wrapping.

### 6.5 Text Control

| Feature | CSS |
|---------|-----|
| Shrink to fit | `mso-text-control:shrinktofit` |
| Text rotation (degrees) | `mso-rotate:90` (or any degree value) |
| Vertical text (no rotation) | `layout-flow:vertical` |

### 6.6 Merged Cells

Use standard HTML `colspan` and `rowspan` attributes:

```html
<td colspan=7 height=35 class=xl74 width=609
    style='border-right:1.0pt solid black;height:26.0pt;width:455pt'>
    April 2026
</td>
```

For merged cells, the merged cell element needs border overrides as inline styles for any edges that the class doesn't cover (e.g., a cell spanning to the right table edge needs `border-right` even if the class only defines `border-left`).

---

## 7. Number Formatting

### 7.1 The `mso-number-format` Property

Without `mso-number-format`, Excel treats pasted values as plain text — currency and percentage values won't be recognized as numbers. The property is set in CSS (either in the style block or inline).

### 7.2 Built-in Format Names

These are predefined names Excel recognizes directly:

| Format Name | Display Example | CSS |
|-------------|----------------|-----|
| General | Auto-detect | `mso-number-format:General` |
| Percent | 15.60% | `mso-number-format:Percent` |
| Short Date | 01/03/1998 | `mso-number-format:"Short Date"` |
| Medium Date | 01-Mar-98 | `mso-number-format:"Medium Date"` |
| Short Time | 5:16 | `mso-number-format:"Short Time"` |
| Medium Time | 5:16 AM | `mso-number-format:"Medium Time"` |
| Long Time | 5:16:21 | `mso-number-format:"Long Time"` |

### 7.3 Custom Format Strings

Custom format strings use Excel's number format codes, with special characters escaped by backslashes in CSS:

| Format | CSS Value | Display |
|--------|-----------|---------|
| No decimals | `"0"` | 1235 |
| 3 decimals | `"0\.000"` | 1234.568 |
| Thousands + 3 dec | `"\#\,\#\#0\.000"` | 1,234.568 |
| Fractions | `"#\ ???/???"` | 1/8 |
| Text (preserve as-is) | `"\@"` | 12345 (stored as text) |
| Percent, no decimals | `"0%"` | 16% |
| Scientific notation | `"0.E+00"` | 1.E+03 |
| Date (mm/dd/yy) | `"mm\/dd\/yy"` | 01/03/98 |
| Date (mmmm d, yyyy) | `"mmmm\ d\,\ yyyy"` | January 3, 1998 |
| Date (d-mmm-yyyy) | `"d\-mmm\-yyyy"` | 3-Jan-1998 |
| DateTime (AM/PM) | `"m\/d\/yy\ h:mm\ AM\/PM"` | 1/3/98 5:16 PM |
| Month-Year title | `"mmmm\\ yyyy"` | April 2026 |
| 2 dec, red negatives | `"\#\,\#\#0\.00_\ \;\[Red\]\-\#\,\#\#0\.00\ "` | 1,234.56 / -1,234.56 (red) |

### 7.4 Currency Format String (Detailed Breakdown)

The currency format used in the template:

```css
mso-number-format:"\0022$\0022\#\,\#\#0_\)\;\[Red\]\\\(\0022$\0022\#\,\#\#0\\\)"
```

Decoded:

| Escape | Meaning |
|--------|---------|
| `\0022` | Unicode escape for `"` (literal double-quote character in CSS) |
| `$` | Dollar sign prefix |
| `\#\,\#\#0` | Number with comma grouping, no decimals |
| `_\)` | Space padding width of `)` for alignment |
| `\;` | Semicolon — separates positive and negative sections |
| `\[Red\]` | Color modifier — display negatives in red |
| `\\\(` / `\\\)` | Literal parentheses around negative values |

### 7.5 Escaping Rules

In CSS `mso-number-format` strings:
- Backslash `\` escapes the next character as literal
- `\0022` is the CSS Unicode escape for `"` (used to embed literal quotes)
- Semicolons, commas, periods, hash signs, and other Excel format metacharacters must be backslash-escaped in the CSS value

### 7.6 Where to Apply Number Formats

Number format can be set:
1. In a class definition in the `<style>` block — applies to all cells of that class
2. As an inline `style=` attribute — overrides the class for a specific cell

Since different cells in the same column can have different formats, inline is often necessary:

```html
<td class=cm style='border-top:none;text-align:right;mso-number-format:Percent;'>15.60%</td>
```

### 7.7 Percent Format Gotcha

When using `mso-number-format:Percent`, Excel interprets the cell value as already being in fraction form. A value of `100` displays as `10000.00%`. To display `100%`, the HTML cell value should be `1`. This matches Excel's internal representation where 100% = 1.0.

### 7.8 `vnd.ms-excel.numberformat` (Legacy)

In Excel 97, the property name was `vnd.ms-excel.numberformat` rather than `mso-number-format`. Both are recognized in modern Excel for backward compatibility, but `mso-number-format` should be used for new content.

---

## 8. Cell Data Types

### 8.1 The `x:num`, `x:str`, `x:bool`, `x:err`, `x:fmla` Attributes

These attributes on `<td>` elements explicitly declare the cell's data type, overriding Excel's auto-detection. They require the `xmlns:x="urn:schemas-microsoft-com:office:excel"` namespace on the `<html>` element.

| Attribute | Purpose | Example |
|-----------|---------|---------|
| `x:num` | Cell contains a number; attribute value is the precise value | `<td x:num="12344.6789">12345</td>` |
| `x:num` (no value) | Cell is numeric; displayed value is the precise value | `<td x:num>12345</td>` |
| `x:str` | Cell contains a string (even if it looks like a number) | `<td x:str>12345</td>` |
| `x:bool` | Cell contains a Boolean | `<td x:bool="TRUE">TRUE</td>` |
| `x:err` | Cell contains an error value | `<td x:err="#DIV/0!">#DIV/0!</td>` |
| `x:fmla` | Cell contains a formula | `<td x:fmla="=SUM(B2:B5)">10</td>` |

The `x:num` attribute is particularly important: the **displayed** text in the `<td>` element may be rounded or formatted (e.g., "12,345"), while the `x:num` attribute carries the full-precision underlying value (e.g., "12344.6789").

### 8.2 Default Data Type Resolution

If the `x:str` attribute is specified on the `<table>` element, all cells without an explicit data type attribute default to string. Without `x:str` on the table, Excel auto-detects each cell's type from its content.

### 8.3 Formulas

Formulas use the `x:fmla` attribute with the `=` prefix:

```html
<td x:fmla="=SUM(B2:B5)" x:num>10</td>
<td x:fmla="=CONCATENATE(&quot;The &quot;, &quot;bicycle&quot;)">The bicycle</td>
```

Character entities (`&quot;`, `&#39;`) must be used for quotes inside formula strings.

---

## 9. Font Handling

### 9.1 Font Properties in CSS

| Property | CSS Syntax | Notes |
|----------|-----------|-------|
| Family | `font-family:"Aptos Display", sans-serif` | Names with spaces must be quoted |
| Size | `font-size:11.0pt` | Always in pt |
| Weight | `font-weight:700` | Use numeric, not `bold` |
| Style | `font-style:italic` | |
| Color | `color:#FF0000` | Use `windowtext` for auto |
| Underline | `text-decoration:underline` | |
| Double underline | `text-decoration:underline; mso-text-underline:double` | |
| Single accounting underline | `text-decoration:underline; mso-text-underline:single-accounting` | |
| Double accounting underline | `text-decoration:underline; mso-text-underline:double-accounting` | |
| Strikethrough | `text-decoration:line-through` | |
| Superscript | `vertical-align:super` | |
| Subscript | `vertical-align:sub` | |
| Outline (Mac only) | `mso-text-effect:outline` | |
| Shadow (Mac only) | `text-shadow:auto` | |

### 9.2 HTML Equivalents for Compatibility

Excel also recognizes HTML elements for backward compatibility with older browsers:

| Formatting | HTML | CSS |
|-----------|------|-----|
| Bold | `<B>` | `font-weight:700` |
| Italic | `<I>` | `font-style:italic` |
| Underline | `<U>` | `text-decoration:underline` |
| Strikethrough | `<STRIKE>` | `text-decoration:line-through` |
| Font name | `<FONT FACE="name">` | `font-family:"name"` |
| Font size | `<FONT SIZE="value">` | `font-size:Npt` |
| Font color | `<FONT COLOR="color">` | `color:value` |

When both HTML elements and CSS styles are present, CSS takes precedence in Excel.

### 9.3 `mso-font-charset` Values

| Value | Charset | Typical Fonts |
|-------|---------|---------------|
| 0 | ANSI_CHARSET | Calibri, Arial, Times New Roman |
| 1 | DEFAULT_CHARSET | Aptos Display, Aptos Narrow |
| 2 | SYMBOL_CHARSET | Symbol, Wingdings |
| 128 | SHIFTJIS_CHARSET | MS Gothic, MS Mincho |
| 136 | CHINESEBIG5_CHARSET | MingLiU |
| 255 | OEM_CHARSET | Terminal |

**When using Aptos-family fonts**, set `mso-font-charset:1` (not `0`). Calibri and Arial use `0`.

### 9.4 Partial Cell Formatting (Rich Text)

When only part of a cell's text has different formatting, the `<span>` element or `<font>` element wraps the formatted fragment within the `<td>`:

```html
<td class=cm>Normal text <font color=red class=xl67>red text</font> normal again</td>
```

---

## 10. Auto Color

### 10.1 The Problem with `auto`

The literal string `auto` should **never** be used as a color value in CSS for Excel HTML. Different browsers interpret it unpredictably. Instead, use the keyword `windowtext` or the explicit hex value.

### 10.2 Auto Color Mappings

| Object | "Auto" Color Representation |
|--------|-----------------------------|
| Font color | `color:windowtext` |
| Border color | `border-{side}:Npt solid windowtext` |
| Background | `mso-background-source:auto` (omit the background color) |
| Pattern foreground | `mso-pattern: .25gray windowtext` |

---

## 11. Table Structure and Row/Column Dimensions

### 11.1 Table Element

```html
<table border=0 cellpadding=0 cellspacing=0 style='border-collapse:collapse'>
```

- `border=0` — all borders come from CSS classes, not the table attribute
- `border-collapse:collapse` — adjacent borders merge (essential for the border system to work)

### 11.2 Column Widths

Column widths are specified with `<col>` elements before the first `<tr>`:

```html
<col width=75 style='mso-width-source:userset;width:56pt'>
<col width=120 style='width:90pt'>
```

The `width` attribute is in pixels; the `style` `width` is in points. The `mso-width-source:userset` property indicates the column was manually sized by the user (as opposed to auto-calculated).

When pasting clipboard HTML, column widths are **not** preserved — Excel ignores them on paste and auto-fits. This has been confirmed by direct testing: `<col>` elements with `mso-width-source:userset`, `mso-width-alt`, `width` attributes, and matching `width`/`style` on header `<td>` elements are all ignored on clipboard paste. Column widths are only applied when opening an HTML file as a workbook (File > Open).

### 11.3 Row Heights

Each `<tr>` should declare height:

```html
<tr height=21 style='height:16.0pt'>
```

Excel's default row height is 16.0pt (21 pixels). For title rows or rows with larger content:

```html
<tr height=35 style='height:26.0pt'>
```

### 11.4 Empty Cells

Excel writes `&nbsp;` in empty cells, not an empty string. This ensures the cell occupies space in the rendered table:

```html
<td class=cm style='border-top:none;'>&nbsp;</td>
```

---

## 12. Cell Protection

The `mso-protection` CSS property controls cell locking and visibility:

```css
mso-protection:locked visible;      /* Default: locked and visible */
mso-protection:unlocked visible;    /* Unlocked (editable when sheet is protected) */
mso-protection:locked hidden;       /* Locked and formula hidden */
```

---

## 13. Merged Title Rows (colspan)

### 13.1 Structure

```html
<td colspan=7 height=35 class=xl74 width=609
    style='border-right:1.0pt solid black;height:26.0pt;width:455pt'>
    April 2026
</td>
```

### 13.2 Rules

- The merged cell needs `border-right` as an **inline style override** because the class only defines the left border
- Only the leftmost cell's `<td>` element is emitted; the remaining cells covered by the colspan are omitted from the HTML
- For date-formatted titles, use `mso-number-format:"mmmm\\ yyyy"` to tell Excel this is a date displayed as "April 2026"
- The header row below the title needs `border-top:none` to avoid a doubled border

---

## 14. Conditional Formatting via CSS

When a cell's formatting results from a conditional format or number format rule (e.g., negative numbers appear in red), the `mso-ignore:style` property indicates this:

```css
.xl67 {color:red; mso-ignore:style}
```

This tells Excel the color isn't a "real" style but a result of a formatting condition, so it should re-evaluate based on the cell's value and number format.

---

## 15. Platform Differences

### 15.1 Windows vs. macOS

| Aspect | Windows | macOS |
|--------|---------|-------|
| Clipboard type | `CF_HTML` (registered as "HTML Format") | `public.html` (NSPasteboard) |
| Envelope format | Byte-offset header + HTML | Raw HTML (no offset header) |
| `<!--StartFragment-->` | Required (defines paste boundary) | Recognized but not strictly required |
| Excel version identifier | "Microsoft Excel 15" | Same |
| Default font (modern Excel) | Aptos Narrow / Calibri | Same |
| `mso-font-charset` for Aptos | `1` | `1` |

### 15.2 Excel Online (Web)

Excel Online also reads HTML from the clipboard but wraps it with additional metadata attributes:

```html
<div ccp_infra_version='3'
     ccp_infra_timestamp='1603806103470'
     ccp_infra_user_hash='804634202'
     data-ccp-timestamp='1603806103470'>
  <html>...</html>
</div>
```

These `ccp_infra_*` attributes are specific to Office Online's clipboard infrastructure. When generating HTML for paste into Excel Online, they can be omitted — the standard Office HTML structure works.

### 15.3 Google Sheets Paste Behavior

Google Sheets has a much more limited HTML paste parser than Excel. It does **not** recognize `mso-*` properties, Office XML namespaces, or the `ProgId` meta tag. When targeting both Excel and Google Sheets, a dual-format approach or lowest-common-denominator HTML may be necessary.

---

## 16. Complete Template Example

### 16.1 JSON Input Format

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
    "default_font": "Aptos Display",
    "default_font_size": "11"
  }
}
```

### 16.2 Style Object Keys

| Key | Default | Description |
|-----|---------|-------------|
| `style.header_bg` | `#4472C4` | Header row background |
| `style.header_fg` | `#FFFFFF` | Header row text color |
| `style.total_bg` | `#F2F2F2` | Total (last) row background |
| `style.default_font` | `Calibri` | Font family |
| `style.default_font_size` | `11` | Font size in pt |

### 16.3 Per-Cell Style Object

| Field | Type | Description |
|-------|------|-------------|
| `alignment` | `"left"`, `"center"`, `"right"` | Text alignment |
| `bold` | boolean | Bold text |
| `fg_color` | hex string | Text color |
| `bg_color` | hex string | Background (triggers total-row class for last row) |
| `number_format` | `"currency"`, `"percent"` | Excel number format; omit for General |

---

## 17. Implementation Pitfalls

### 17.1 `Option::None` serializes as JSON `null`

In Rust with serde, `Option::None` becomes JSON `null`. In minijinja, `null` is a *defined* value, so `{{ field | default('fallback') }}` does NOT trigger — it renders the literal string `"none"`. This causes bugs like `background-color:none` and `font-size:nonept`.

**Fix:** Add `#[serde(skip_serializing_if = "Option::is_none")]` to all `Option<T>` fields. This omits the field entirely from JSON when `None`, making minijinja see it as undefined.

### 17.2 Top-level JSON keys outside the expected struct are dropped

Any key not matching a struct field is silently discarded during deserialization. Template variables like font and size must live inside the `style` object, not at the top level.

### 17.3 Font names with spaces need CSS quotes

`font-family:Aptos Display, sans-serif` is invalid — only `Aptos` is recognized as the font name. Must be: `font-family:"Aptos Display", sans-serif`.

### 17.4 `text-align` needs dual declaration

Covered in §6.1. The class-level `text-align:general` takes CSS cascade priority over the HTML `align=` attribute.

### 17.5 `background-color` vs `background`

Covered in §4.1. Use `background:`, never `background-color:`.

### 17.6 `font-weight:bold` vs `font-weight:700`

Excel's CSS parser does not reliably interpret the keyword `bold`. Use the numeric value `700`.

### 17.7 `mso-pattern` must match fill state

Cells WITH a background: `mso-pattern:black none`  
Cells WITHOUT a background: `mso-pattern:auto`

Mixing these up causes fills to disappear or phantom fills to appear.

---

## 18. Open Questions and Uncertainties

### 18.1 `mso-number-format` Locale Sensitivity

Custom number format strings use the US English format code syntax (`#,##0.00`). When the format string is pasted into an Excel instance running under a different locale (e.g., German where `,` is the decimal separator), behavior is inconsistent. The `mso-displayed-decimal-separator` and `mso-displayed-thousand-separator` on the `<table>` element help Excel interpret displayed values, but the interaction between these and `mso-number-format` custom strings under non-English locales is not fully documented. Testing under target locales is recommended.

### 18.2 Elapsed Time Formats

The format `[h]:mm` (elapsed hours) is represented in Excel's generated CSS as `\[h\]\:mm`, but applying this exact string via `mso-number-format` has been reported to produce unexpected results (generating `[h]:mm:ss` instead). This is a known edge case without a reliable workaround documented in the community.

### 18.3 Column Widths on Paste vs. Open — CONFIRMED

Column widths specified via `<col>` elements are honored when Excel opens an HTML file as a workbook, but are **completely ignored** when pasting clipboard HTML. This has been directly tested (2026-03-26) with `<col>` elements using `mso-width-source:userset`, `mso-width-alt`, pixel `width` attributes, and matching `width`/`style` on header `<td>` elements — none had any effect on paste. Excel auto-fits columns on paste regardless. This is documented Excel behavior, not a bug. Workarounds: auto-fit after paste, or open the HTML as a file instead of pasting.

### 18.4 Clipboard HTML Length Limits

There is no officially documented maximum size for clipboard HTML. In practice, very large tables (thousands of rows) may fail to paste or cause performance issues. Excel's XML Spreadsheet clipboard format is more efficient for large data sets.

### 18.5 Version Differences Across Excel 2016, 2019, 2021, 365

The `ProgId=Excel.Sheet` and `Generator=Microsoft Excel 15` values have remained consistent across Excel 2016 through Excel 365 (all report as "Excel 15" internally). No behavioral differences in the HTML paste parser have been documented between these versions, but edge cases in `mso-number-format` or border rendering may exist.

### 18.6 Conditional Formatting and Sparklines

Conditional formatting rules (data bars, icon sets, color scales) cannot be transmitted through clipboard HTML. Only the *rendered* appearance (the specific color resulting from the rule) can be captured. Sparklines are also not representable in HTML clipboard format.

### 18.7 Images and Charts

VML-based images can theoretically be embedded using `<v:imagedata>` within cells, but clipboard paste of images through HTML is unreliable. Charts are not representable in HTML clipboard format; Excel places them separately as image formats (EMF, PNG) on the clipboard.

---

## 19. Reference: Capturing Excel's Actual Output

To capture a fresh reference of what Excel puts on the clipboard:

```bash
# macOS (using clipli or similar tool)
# Copy cells in Excel, then:
clipli read --type html > excel_reference.html

# macOS (using Pasteboard Viewer app)
# Install from App Store, copy cells, inspect public.html type

# Windows (using PowerShell)
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.Clipboard]::GetData("HTML Format") | Out-File clipboard.html
```

Key observations from real Excel 15 output:
- Every unique formatting combination gets its own numbered class (`.xl65`, `.xl66`, etc.)
- The base `td` rule defines ALL default properties
- `mso-pattern:black none` for cells WITH background; `mso-pattern:auto` for cells WITHOUT
- `windowtext` for default text/border color
- Row heights and column widths are explicit
- `<col>` elements define column widths before the first `<tr>`
- `<!--[if gte mso 9]>` blocks contain optional workbook metadata