// Excel HTML generator — converts CSV to Excel-native clipboard HTML.
//
// Supports two styles:
//   "table" — Excel Table format (banded rows, #8EA9DB borders, full inline styles)
//   "plain" — Plain range format (windowtext borders, thick outer frame)
//
// Full CLI feature set: column formatting, conditional colors, hyperlinks,
// word wrap, title row, total row, column selection/ordering/renaming/hiding.

use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ExcelConfig {
    pub style: TableStyle,
    pub header_bg: String,
    pub header_fg: String,
    pub band_bg: String,
    pub font: String,
    pub font_size: String,
    /// Column name → (number_format, alignment)
    pub col_formats: HashMap<String, ColFormat>,
    /// Columns to render as bold
    pub bold_cols: Vec<String>,
    /// Columns to render as italic
    pub italic_cols: Vec<String>,
    /// Columns with word wrap enabled
    pub wrap_cols: Vec<String>,
    /// Column → text color hex
    pub fg_colors: HashMap<String, String>,
    /// Column → background color hex
    pub bg_colors: HashMap<String, String>,
    /// Column → alignment (when no format needed)
    pub align_overrides: HashMap<String, String>,
    /// Column → URL pattern with {} as placeholder for cell value
    pub links: HashMap<String, String>,
    /// Conditional color rules
    pub color_rules: Vec<ColorRule>,
    /// Merged title row text
    pub title: Option<String>,
    /// Auto-sum numeric columns as last row
    pub total_row: bool,
    /// Custom row height in pixels
    pub row_height: Option<u32>,
    /// Custom header row height in pixels
    pub header_height: Option<u32>,
    /// Column selection and ordering (None = all columns in CSV order)
    pub columns: Option<Vec<String>>,
    /// Columns to hide from output
    pub hidden_cols: Vec<String>,
    /// Column renames: old_name → new_name
    pub renames: HashMap<String, String>,
    /// Column → font size override
    pub col_font_sizes: HashMap<String, String>,
    /// Use SUM formulas in total row instead of pre-computed values
    pub total_formula: bool,
    /// Per-cell formulas: (column_name, row_index) → formula string
    /// Row index is 0-based (data rows, not counting header)
    pub cell_formulas: HashMap<(String, usize), String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TableStyle {
    Table, // Excel Table (banded, blue-gray borders)
    Plain, // Plain range (thick outer, thin inner, windowtext)
}

#[derive(Debug, Clone)]
pub struct ColFormat {
    pub number_format: Option<String>,
    pub alignment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ColorRule {
    pub column: String,
    pub operator: String, // >=, <=, >, <, ==, !=, contains, empty, not_empty
    pub value: String,
    pub bg_color: String,
    pub fg_color: String,
}

impl Default for ExcelConfig {
    fn default() -> Self {
        Self {
            style: TableStyle::Table,
            header_bg: "#4472C4".to_string(),
            header_fg: "#FFFFFF".to_string(),
            band_bg: "#D9E1F2".to_string(),
            font: "Calibri".to_string(),
            font_size: "12".to_string(),
            col_formats: HashMap::new(),
            bold_cols: Vec::new(),
            italic_cols: Vec::new(),
            wrap_cols: Vec::new(),
            fg_colors: HashMap::new(),
            bg_colors: HashMap::new(),
            align_overrides: HashMap::new(),
            links: HashMap::new(),
            color_rules: Vec::new(),
            title: None,
            total_row: false,
            row_height: None,
            header_height: None,
            columns: None,
            hidden_cols: Vec::new(),
            renames: HashMap::new(),
            col_font_sizes: HashMap::new(),
            total_formula: false,
            cell_formulas: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// CSV parsing
// ---------------------------------------------------------------------------

pub fn read_csv(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>), Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let headers: Vec<String> = rdr.headers()?.iter().map(|s| s.to_string()).collect();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for result in rdr.records() {
        let record = result?;
        rows.push(record.iter().map(|s| s.to_string()).collect());
    }
    Ok((headers, rows))
}

pub fn read_csv_from_str(data: &str) -> Result<(Vec<String>, Vec<Vec<String>>), Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_reader(data.as_bytes());
    let headers: Vec<String> = rdr.headers()?.iter().map(|s| s.to_string()).collect();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for result in rdr.records() {
        let record = result?;
        rows.push(record.iter().map(|s| s.to_string()).collect());
    }
    Ok((headers, rows))
}

// ---------------------------------------------------------------------------
// Column resolution — selection, ordering, hiding
// ---------------------------------------------------------------------------

/// Resolve which columns to include and in what order.
/// Returns indices into the original CSV headers.
fn resolve_columns(
    csv_headers: &[String],
    config: &ExcelConfig,
) -> Vec<usize> {
    let indices: Vec<usize> = if let Some(ref cols) = config.columns {
        // Use specified order — look up each name in csv_headers
        cols.iter()
            .filter_map(|name| csv_headers.iter().position(|h| h == name))
            .collect()
    } else {
        (0..csv_headers.len()).collect()
    };

    // Remove hidden columns
    indices
        .into_iter()
        .filter(|&i| !config.hidden_cols.contains(&csv_headers[i]))
        .collect()
}

/// Get the display name for a column (after renames).
fn display_name(csv_name: &str, config: &ExcelConfig) -> String {
    config
        .renames
        .get(csv_name)
        .cloned()
        .unwrap_or_else(|| csv_name.to_string())
}

// ---------------------------------------------------------------------------
// Number format CSS
// ---------------------------------------------------------------------------

/// Return the mso-number-format CSS string for a format name (owned version).
pub fn number_format_css_owned(fmt: &str) -> String {
    number_format_css(fmt).to_string()
}

fn number_format_css(fmt: &str) -> &'static str {
    match fmt {
        "currency" => r#"mso-number-format:"\0022$\0022\#\,\#\#0_\)\;\[Red\]\\\(\0022$\0022\#\,\#\#0\\\)";"#,
        "accounting" => r#"mso-number-format:"_\(* \#\,\#\#0_\)\;_\(* \\\(\#\,\#\#0\\\)\;_\(* \0022-\0022??_\)\;_\(\@_\)";"#,
        "percent" => "mso-number-format:Percent;",
        "percent_int" => "mso-number-format:0%;",
        "percent_1dp" => r#"mso-number-format:"0\.0%";"#,
        "integer" => r#"mso-number-format:"\#\,\#\#0";"#,
        "standard" => "mso-number-format:Standard;",
        "text" => r#"mso-number-format:"\@";"#,
        "datetime_iso" => r#"mso-number-format:"yyyy\\-mm\\-dd\\ hh\:mm";"#,
        _ => "",
    }
}

// ---------------------------------------------------------------------------
// Conditional color evaluation
// ---------------------------------------------------------------------------

/// Parse a numeric value from a cell, stripping $, %, commas.
fn parse_numeric(s: &str) -> Option<f64> {
    let cleaned: String = s
        .trim()
        .replace(['$', '%', ','], "")
        .replace('(', "-")
        .replace(')', "");
    cleaned.parse::<f64>().ok()
}

/// Evaluate a color rule against a cell value.
fn evaluate_rule(rule: &ColorRule, cell_value: &str) -> bool {
    match rule.operator.as_str() {
        "empty" => cell_value.trim().is_empty(),
        "not_empty" => !cell_value.trim().is_empty(),
        "contains" => cell_value.contains(&rule.value),
        "==" | "eq" => cell_value.trim() == rule.value,
        "!=" | "ne" => cell_value.trim() != rule.value,
        ">=" => {
            match (parse_numeric(cell_value), parse_numeric(&rule.value)) {
                (Some(a), Some(b)) => a >= b,
                _ => false,
            }
        }
        "<=" => {
            match (parse_numeric(cell_value), parse_numeric(&rule.value)) {
                (Some(a), Some(b)) => a <= b,
                _ => false,
            }
        }
        ">" => {
            match (parse_numeric(cell_value), parse_numeric(&rule.value)) {
                (Some(a), Some(b)) => a > b,
                _ => false,
            }
        }
        "<" => {
            match (parse_numeric(cell_value), parse_numeric(&rule.value)) {
                (Some(a), Some(b)) => a < b,
                _ => false,
            }
        }
        _ => false,
    }
}

/// Find the first matching color rule for a column/value, returning (bg, fg).
fn find_matching_rule<'a>(
    rules: &'a [ColorRule],
    col_name: &str,
    cell_value: &str,
) -> Option<(&'a str, &'a str)> {
    rules
        .iter()
        .filter(|r| r.column == col_name)
        .find(|r| evaluate_rule(r, cell_value))
        .map(|r| (r.bg_color.as_str(), r.fg_color.as_str()))
}

// ---------------------------------------------------------------------------
// Total row computation
// ---------------------------------------------------------------------------

fn compute_total_row(
    headers: &[String],
    rows: &[Vec<String>],
    col_indices: &[usize],
    config: &ExcelConfig,
) -> Vec<String> {
    let mut totals: Vec<String> = Vec::with_capacity(col_indices.len());

    for (pos, &idx) in col_indices.iter().enumerate() {
        if pos == 0 {
            totals.push("Total".to_string());
            continue;
        }

        let col_name = &headers[idx];
        let cf = config.col_formats.get(col_name);
        let has_numeric_format = cf
            .and_then(|f| f.number_format.as_deref())
            .map(|nf| matches!(nf, "currency" | "accounting" | "integer" | "standard" | "percent" | "percent_int" | "percent_1dp"))
            .unwrap_or(false);

        // Try to sum the column
        let mut sum = 0.0f64;
        let mut any_numeric = false;
        for row in rows {
            if let Some(val) = row.get(idx) {
                if let Some(n) = parse_numeric(val) {
                    sum += n;
                    any_numeric = true;
                }
            }
        }

        if any_numeric && has_numeric_format {
            // Format the sum based on number format
            let nf = cf.and_then(|f| f.number_format.as_deref()).unwrap_or("");
            totals.push(format_total(sum, nf));
        } else {
            totals.push(String::new());
        }
    }

    totals
}

/// Convert a 0-based column index to an Excel column letter (0=A, 25=Z, 26=AA, ...).
fn col_letter(idx: usize) -> String {
    let mut result = String::new();
    let mut n = idx;
    loop {
        result.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    result
}

fn format_total(value: f64, format: &str) -> String {
    match format {
        "currency" => format!("${}", format_commas(value as i64)),
        "accounting" => format!("${}", format_commas(value as i64)),
        "integer" => format_commas(value as i64),
        "percent_int" | "percent" | "percent_1dp" => {
            // Percentages: average makes more sense than sum
            format!("{:.0}%", value)
        }
        _ => format!("{}", value),
    }
}

fn format_commas(n: i64) -> String {
    let neg = n < 0;
    let s = n.unsigned_abs().to_string();
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::new();
    let len = chars.len();
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*ch);
    }
    if neg {
        format!("-{}", result)
    } else {
        result
    }
}

// ---------------------------------------------------------------------------
// Per-cell style assembly
// ---------------------------------------------------------------------------

struct CellProps<'a> {
    value: &'a str,
    #[allow(dead_code)]
    col_name: &'a str,
    is_bold: bool,
    is_italic: bool,
    is_wrap: bool,
    fg_color: Option<&'a str>,
    bg_color: Option<&'a str>,
    alignment: Option<&'a str>,
    number_format: &'a str,
    font_size_override: Option<&'a str>,
    link_url: Option<String>,
    formula: Option<&'a str>,
}

fn resolve_cell_props<'a>(
    value: &'a str,
    col_name: &'a str,
    row_idx: usize,
    config: &'a ExcelConfig,
) -> CellProps<'a> {
    let cf = config.col_formats.get(col_name);
    let nf = cf.and_then(|f| f.number_format.as_deref()).unwrap_or("");
    let fmt_align = cf.and_then(|f| f.alignment.as_deref());
    let override_align = config.align_overrides.get(col_name).map(|s| s.as_str());
    let alignment = fmt_align.or(override_align);

    // Color: conditional rules take priority, then static colors
    let (rule_bg, rule_fg) = find_matching_rule(&config.color_rules, col_name, value)
        .map(|(bg, fg)| (Some(bg), Some(fg)))
        .unwrap_or((None, None));

    let fg_color = rule_fg.or_else(|| config.fg_colors.get(col_name).map(|s| s.as_str()));
    let bg_color = rule_bg.or_else(|| config.bg_colors.get(col_name).map(|s| s.as_str()));

    let link_url = config.links.get(col_name).map(|pattern| {
        pattern.replace("{}", value)
    });

    let formula = config
        .cell_formulas
        .get(&(col_name.to_string(), row_idx))
        .map(|s| s.as_str());

    CellProps {
        value,
        col_name,
        is_bold: config.bold_cols.iter().any(|b| b == col_name),
        is_italic: config.italic_cols.iter().any(|b| b == col_name),
        is_wrap: config.wrap_cols.iter().any(|b| b == col_name),
        fg_color,
        bg_color,
        alignment,
        number_format: nf,
        font_size_override: config.col_font_sizes.get(col_name).map(|s| s.as_str()),
        link_url,
        formula,
    }
}

// ---------------------------------------------------------------------------
// HTML generation
// ---------------------------------------------------------------------------

pub fn generate_html(
    headers: &[String],
    rows: &[Vec<String>],
    config: &ExcelConfig,
) -> String {
    let col_indices = resolve_columns(headers, config);
    let ncols = col_indices.len();
    let charset = if config.font.contains("Aptos") { "1" } else { "0" };
    let rh = config.row_height.unwrap_or(21);
    let rh_pt = (rh as f32 * 0.75).round();
    let hh = config.header_height.unwrap_or(21);
    let hh_pt = (hh as f32 * 0.75).round();

    // Compute total row if requested
    let total_row_data = if config.total_row {
        Some(compute_total_row(headers, rows, &col_indices, config))
    } else {
        None
    };

    let total_rows = rows.len() + if total_row_data.is_some() { 1 } else { 0 };
    let mut html = String::with_capacity(total_rows * ncols * 300);

    // Envelope + style block
    write_envelope(&mut html);
    html.push_str("<style>\n<!--table\n\t{mso-displayed-decimal-separator:\"\\.\";");
    html.push_str("\n\tmso-displayed-thousand-separator:\"\\,\";}\n");
    html.push_str("@page\n\t{margin:.75in .7in .75in .7in;\n\tmso-header-margin:.3in;\n\tmso-footer-margin:.3in;}\n");
    html.push_str("tr\n\t{mso-height-source:auto;}\ncol\n\t{mso-width-source:auto;}\n");
    html.push_str("br\n\t{mso-data-placement:same-cell;}\n");
    write_base_td(&mut html, &config.font, &config.font_size, charset);

    match config.style {
        TableStyle::Table => {
            // Minimal classes for table style — heavy lifting is in inline styles
        }
        TableStyle::Plain => {
            write_plain_classes(&mut html, &config.header_bg, &config.header_fg);
        }
    }

    html.push_str("-->\n</style>\n</head>\n<body>\n");
    html.push_str("<table border=0 cellpadding=0 cellspacing=0 style='border-collapse:collapse'>\n");
    html.push_str("<!--StartFragment-->\n");

    // Title row
    if let Some(ref title_text) = config.title {
        write_title_row(&mut html, title_text, ncols, config);
    }

    // Header row
    let has_title = config.title.is_some();
    write_header_row(&mut html, headers, &col_indices, config, ncols, hh, hh_pt, has_title);

    // Data rows
    let is_table = config.style == TableStyle::Table;
    for (row_idx, row) in rows.iter().enumerate() {
        let is_last = row_idx == rows.len() - 1 && total_row_data.is_none();
        write_data_row(&mut html, row, headers, &col_indices, config, ncols, rh, rh_pt, is_last, is_table, row_idx);
    }

    // Total row
    if let Some(ref total_data) = total_row_data {
        write_total_row(&mut html, total_data, headers, &col_indices, config, ncols, rh, rh_pt, is_table, rows.len());
    }

    html.push_str("<!--EndFragment-->\n</table>\n</body>\n</html>");
    html
}

fn write_envelope(html: &mut String) {
    html.push_str(
        "<html xmlns:v=\"urn:schemas-microsoft-com:vml\"\n\
         xmlns:o=\"urn:schemas-microsoft-com:office:office\"\n\
         xmlns:x=\"urn:schemas-microsoft-com:office:excel\"\n\
         xmlns=\"http://www.w3.org/TR/REC-html40\">\n\
         <head>\n\
         <meta http-equiv=Content-Type content=\"text/html; charset=utf-8\">\n\
         <meta name=ProgId content=Excel.Sheet>\n\
         <meta name=Generator content=\"clipli\">\n"
    );
}

fn write_base_td(html: &mut String, fnt: &str, fsz: &str, charset: &str) {
    html.push_str(&format!(
        "td\n\
         \t{{padding-top:1px;\n\
         \tpadding-right:1px;\n\
         \tpadding-left:1px;\n\
         \tmso-ignore:padding;\n\
         \tcolor:black;\n\
         \tfont-size:{fsz}.0pt;\n\
         \tfont-weight:400;\n\
         \tfont-style:normal;\n\
         \ttext-decoration:none;\n\
         \tfont-family:\"{fnt}\", sans-serif;\n\
         \tmso-font-charset:{charset};\n\
         \tmso-number-format:General;\n\
         \ttext-align:general;\n\
         \tvertical-align:bottom;\n\
         \tborder:none;\n\
         \tmso-background-source:auto;\n\
         \tmso-pattern:auto;\n\
         \tmso-protection:locked visible;\n\
         \twhite-space:nowrap;\n\
         \tmso-rotate:0;}}\n"
    ));
}

fn write_plain_classes(html: &mut String, hdr_bg: &str, hdr_fg: &str) {
    html.push_str(&format!(".hdr_l\n\t{{color:{hdr_fg};font-weight:700;text-align:center;vertical-align:middle;border-top:1.0pt solid windowtext;border-right:none;border-bottom:.5pt solid windowtext;border-left:1.0pt solid windowtext;background:{hdr_bg};mso-pattern:black none;}}\n"));
    html.push_str(&format!(".hdr_m\n\t{{color:{hdr_fg};font-weight:700;text-align:center;vertical-align:middle;border-top:1.0pt solid windowtext;border-right:none;border-bottom:.5pt solid windowtext;border-left:none;background:{hdr_bg};mso-pattern:black none;}}\n"));
    html.push_str(&format!(".hdr_r\n\t{{color:{hdr_fg};font-weight:700;text-align:center;vertical-align:middle;border-top:1.0pt solid windowtext;border-right:1.0pt solid windowtext;border-bottom:.5pt solid windowtext;border-left:none;background:{hdr_bg};mso-pattern:black none;}}\n"));
    html.push_str(".cl\n\t{border-top:none;border-right:none;border-bottom:.5pt solid windowtext;border-left:1.0pt solid windowtext;}\n");
    html.push_str(".cm\n\t{border-top:none;border-right:none;border-bottom:.5pt solid windowtext;border-left:none;}\n");
    html.push_str(".cr\n\t{border-top:none;border-right:1.0pt solid windowtext;border-bottom:.5pt solid windowtext;border-left:none;}\n");
    html.push_str(".tl\n\t{border-top:none;border-right:none;border-bottom:1.0pt solid windowtext;border-left:1.0pt solid windowtext;background:#F2F2F2;mso-pattern:black none;}\n");
    html.push_str(".tm\n\t{border-top:none;border-right:none;border-bottom:1.0pt solid windowtext;border-left:none;background:#F2F2F2;mso-pattern:black none;}\n");
    html.push_str(".tr\n\t{border-top:none;border-right:1.0pt solid windowtext;border-bottom:1.0pt solid windowtext;border-left:none;background:#F2F2F2;mso-pattern:black none;}\n");
}

// ---------------------------------------------------------------------------
// Row writers
// ---------------------------------------------------------------------------

fn write_title_row(html: &mut String, title: &str, ncols: usize, config: &ExcelConfig) {
    let fnt = &config.font;
    let border = match config.style {
        TableStyle::Table => ".5pt solid #8EA9DB",
        TableStyle::Plain => "1.0pt solid windowtext",
    };
    html.push_str(&format!(
        " <tr height=36 style='height:27.0pt'>\n\
         \x20 <td colspan={ncols} style='font-size:20.0pt;font-weight:700;\
         text-align:center;vertical-align:middle;\
         font-family:\"{fnt}\", sans-serif;\
         border-top:{border};border-right:{border};\
         border-bottom:{border};border-left:{border}'>{title}</td>\n\
         \x20</tr>\n"
    ));
}

fn write_header_row(
    html: &mut String,
    headers: &[String],
    col_indices: &[usize],
    config: &ExcelConfig,
    ncols: usize,
    hh: u32,
    hh_pt: f32,
    has_title: bool,
) {
    html.push_str(&format!(" <tr height={hh} style='height:{hh_pt:.1}pt'>\n"));

    match config.style {
        TableStyle::Table => {
            let border = ".5pt solid #8EA9DB";
            let hdr_bg = &config.header_bg;
            let hdr_fg = &config.header_fg;
            let fnt = &config.font;
            let fsz = &config.font_size;
            let bt = if has_title { "none" } else { border };

            for (pos, &idx) in col_indices.iter().enumerate() {
                let name = display_name(&headers[idx], config);
                let bl = if pos == 0 { border } else { "none" };
                let br = if pos == ncols - 1 { border } else { "none" };
                html.push_str(&format!(
                    "  <td style='font-size:{fsz}.0pt;color:{hdr_fg};font-weight:700;\
                     text-decoration:none;text-underline-style:none;text-line-through:none;\
                     font-family:\"{fnt}\", sans-serif;\
                     border-top:{bt};border-right:{br};border-bottom:{border};border-left:{bl};\
                     background:{hdr_bg};mso-pattern:{hdr_bg} none'>{name}</td>\n"
                ));
            }
        }
        TableStyle::Plain => {
            for (pos, &idx) in col_indices.iter().enumerate() {
                let name = display_name(&headers[idx], config);
                let cls = if pos == 0 { "hdr_l" } else if pos == ncols - 1 { "hdr_r" } else { "hdr_m" };
                let bt_override = if has_title { " style='border-top:none'" } else { "" };
                html.push_str(&format!("  <td class={cls}{bt_override}>{name}</td>\n"));
            }
        }
    }
    html.push_str(" </tr>\n");
}

fn write_data_row(
    html: &mut String,
    row: &[String],
    headers: &[String],
    col_indices: &[usize],
    config: &ExcelConfig,
    ncols: usize,
    rh: u32,
    rh_pt: f32,
    is_last: bool,
    is_table: bool,
    row_idx: usize,
) {
    html.push_str(&format!(" <tr height={rh} style='height:{rh_pt:.1}pt'>\n"));

    for (pos, &idx) in col_indices.iter().enumerate() {
        let value = row.get(idx).map(|s| s.as_str()).unwrap_or("");
        let col_name = &headers[idx];
        let props = resolve_cell_props(value, col_name, row_idx, config);

        if is_table {
            write_table_cell(html, &props, pos, ncols, config, is_last);
        } else {
            write_plain_cell(html, &props, pos, ncols, config, is_last, false);
        }
    }
    html.push_str(" </tr>\n");
}

fn write_total_row(
    html: &mut String,
    total_data: &[String],
    headers: &[String],
    col_indices: &[usize],
    config: &ExcelConfig,
    ncols: usize,
    rh: u32,
    rh_pt: f32,
    is_table: bool,
    num_data_rows: usize,
) {
    html.push_str(&format!(" <tr height={rh} style='height:{rh_pt:.1}pt'>\n"));

    // Excel row numbers: header is row 1 (or row 2 if title exists), data starts after
    let header_excel_row = if config.title.is_some() { 2 } else { 1 };
    let data_start_row = header_excel_row + 1;
    let data_end_row = data_start_row + num_data_rows - 1;

    for (pos, &idx) in col_indices.iter().enumerate() {
        let value = total_data.get(pos).map(|s| s.as_str()).unwrap_or("");
        let col_name = &headers[idx];

        // Total row: always bold, keep number format, no conditional colors
        let cf = config.col_formats.get(col_name);
        let nf = cf.and_then(|f| f.number_format.as_deref()).unwrap_or("");
        let fmt_align = cf.and_then(|f| f.alignment.as_deref());
        let override_align = config.align_overrides.get(col_name).map(|s| s.as_str());
        let alignment = fmt_align.or(override_align);

        // Build formula for this cell if --total-formula and it's a numeric column
        let formula = if config.total_formula && pos > 0 && !value.is_empty() {
            let col_ltr = col_letter(pos);
            Some(format!("=SUM({col_ltr}{data_start_row}:{col_ltr}{data_end_row})"))
        } else {
            None
        };

        let props = CellProps {
            value,
            col_name,
            is_bold: true,
            is_italic: false,
            is_wrap: false,
            fg_color: None,
            bg_color: Some("#F2F2F2"),
            alignment,
            number_format: nf,
            font_size_override: None,
            link_url: None,
            formula: formula.as_deref(),
        };

        if is_table {
            write_table_cell(html, &props, pos, ncols, config, true);
        } else {
            write_plain_cell(html, &props, pos, ncols, config, true, true);
        }
    }
    html.push_str(" </tr>\n");
}

// ---------------------------------------------------------------------------
// Cell writers
// ---------------------------------------------------------------------------

fn write_table_cell(
    html: &mut String,
    props: &CellProps,
    pos: usize,
    ncols: usize,
    config: &ExcelConfig,
    _is_last: bool,
) {
    let border = ".5pt solid #8EA9DB";
    let fnt = &config.font;
    let fsz = props.font_size_override.unwrap_or(&config.font_size);
    let bg = props.bg_color.unwrap_or(&config.band_bg);
    let fg = props.fg_color.unwrap_or("black");
    let fw = if props.is_bold { "700" } else { "400" };
    let fi = if props.is_italic { "italic" } else { "normal" };
    let bl = if pos == 0 { border } else { "none" };
    let br = if pos == ncols - 1 { border } else { "none" };

    let mut extra = String::new();
    if let Some(align) = props.alignment {
        extra.push_str(&format!("text-align:{align};"));
    }
    if props.is_wrap {
        extra.push_str("white-space:normal;");
    }
    let nf_css = number_format_css(props.number_format);
    if !nf_css.is_empty() {
        extra.push_str(nf_css);
    }

    let align_attr = props.alignment.map(|a| format!(" align={a}")).unwrap_or_default();
    let fmla_attr = props.formula.map(|f| {
        let escaped = f.replace('&', "&amp;").replace('"', "&quot;");
        format!(" x:fmla=\"{escaped}\"")
    }).unwrap_or_default();
    let num_attr = if props.formula.is_some() { " x:num" } else { "" };
    let cell_val = render_cell_value(props, fnt, fsz, fg);

    html.push_str(&format!(
        "  <td{fmla_attr}{num_attr} style='font-size:{fsz}.0pt;color:{fg};font-weight:{fw};font-style:{fi};\
         text-decoration:none;text-underline-style:none;text-line-through:none;\
         font-family:\"{fnt}\", sans-serif;\
         border-top:{border};border-right:{br};border-bottom:{border};border-left:{bl};\
         background:{bg};mso-pattern:{bg} none;{extra}'{align_attr}>{cell_val}</td>\n"
    ));
}

fn write_plain_cell(
    html: &mut String,
    props: &CellProps,
    pos: usize,
    ncols: usize,
    _config: &ExcelConfig,
    is_last: bool,
    is_total: bool,
) {
    let cls = if is_total || is_last {
        if pos == 0 { "tl" } else if pos == ncols - 1 { "tr" } else { "tm" }
    } else {
        if pos == 0 { "cl" } else if pos == ncols - 1 { "cr" } else { "cm" }
    };

    let mut inline = String::from("border-top:none;");
    if props.is_bold { inline.push_str("font-weight:700;"); }
    if props.is_italic { inline.push_str("font-style:italic;"); }
    if let Some(fg) = props.fg_color { inline.push_str(&format!("color:{fg};")); }
    if let Some(bg) = props.bg_color {
        if !is_total {
            inline.push_str(&format!("background:{bg};mso-pattern:black none;"));
        }
    }
    if let Some(align) = props.alignment { inline.push_str(&format!("text-align:{align};")); }
    if props.is_wrap { inline.push_str("white-space:normal;"); }
    let nf_css = number_format_css(props.number_format);
    if !nf_css.is_empty() { inline.push_str(nf_css); }

    let align_attr = props.alignment.map(|a| format!(" align={a}")).unwrap_or_default();
    let fmla_attr = props.formula.map(|f| {
        let escaped = f.replace('&', "&amp;").replace('"', "&quot;");
        format!(" x:fmla=\"{escaped}\"")
    }).unwrap_or_default();
    let num_attr = if props.formula.is_some() { " x:num" } else { "" };
    let cell_val = if props.value.is_empty() { "&nbsp;" } else { props.value };

    html.push_str(&format!(
        "  <td{fmla_attr}{num_attr} class={cls} style='{inline}'{align_attr}>{cell_val}</td>\n"
    ));
}

fn render_cell_value(props: &CellProps, fnt: &str, fsz: &str, fg: &str) -> String {
    let display = if props.value.is_empty() { "&nbsp;" } else { props.value };
    if let Some(ref url) = props.link_url {
        if !props.value.is_empty() {
            return format!(
                "<a href=\"{url}\"><span style='color:{fg};font-size:{fsz}.0pt;\
                 font-family:\"{fnt}\", sans-serif;text-decoration:none'>{display}</span></a>"
            );
        }
    }
    display.to_string()
}

// ---------------------------------------------------------------------------
// CLI parsing helpers
// ---------------------------------------------------------------------------

/// Parse --col flag: NAME:FORMAT or NAME:FORMAT:ALIGN
pub fn parse_col_spec(spec: &str) -> (String, ColFormat) {
    let parts: Vec<&str> = spec.splitn(3, ':').collect();
    let name = parts[0].to_string();
    let number_format = parts.get(1).and_then(|s| {
        if s.is_empty() { None } else { Some(s.to_string()) }
    });
    let alignment = parts.get(2).and_then(|s| {
        if s.is_empty() { None } else { Some(s.to_string()) }
    });
    (name, ColFormat { number_format, alignment })
}

/// Parse --color-if flag: COLUMN:OP:VALUE:BG:FG
/// Examples: "Margin:>=:50:#A0D771:#628048"
///           "Status:==:No:#C92E25:white"
pub fn parse_color_rule(spec: &str) -> Result<ColorRule, String> {
    let parts: Vec<&str> = spec.splitn(5, ':').collect();
    if parts.len() < 5 {
        return Err(format!(
            "invalid --color-if spec '{}': expected COLUMN:OP:VALUE:BG_HEX:FG_HEX",
            spec
        ));
    }
    Ok(ColorRule {
        column: parts[0].to_string(),
        operator: parts[1].to_string(),
        value: parts[2].to_string(),
        bg_color: parts[3].to_string(),
        fg_color: parts[4].to_string(),
    })
}

/// Parse --rename flag: OLD_NAME:NEW_NAME
pub fn parse_rename(spec: &str) -> (String, String) {
    let parts: Vec<&str> = spec.splitn(2, ':').collect();
    let old = parts[0].to_string();
    let new = parts.get(1).map(|s| s.to_string()).unwrap_or_else(|| old.clone());
    (old, new)
}

/// Parse --fg-color or --bg-color: COLUMN:HEX
pub fn parse_color_spec(spec: &str) -> (String, String) {
    let parts: Vec<&str> = spec.splitn(2, ':').collect();
    let col = parts[0].to_string();
    let color = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
    (col, color)
}

/// Parse --col-font-size: COLUMN:SIZE
pub fn parse_col_font_size(spec: &str) -> (String, String) {
    parse_color_spec(spec) // Same format: NAME:VALUE
}

/// Parse --formula flag: COL:ROW:FORMULA
/// Example: "Revenue:5:=SUM(E2:E6)"
pub fn parse_formula_spec(spec: &str) -> Result<(String, usize, String), String> {
    let parts: Vec<&str> = spec.splitn(3, ':').collect();
    if parts.len() < 3 {
        return Err(format!(
            "invalid --formula spec '{}': expected COL:ROW:FORMULA",
            spec
        ));
    }
    let col = parts[0].to_string();
    let row: usize = parts[1].parse().map_err(|_| {
        format!("invalid row index '{}' in --formula spec", parts[1])
    })?;
    let formula = parts[2].to_string();
    Ok((col, row, formula))
}
