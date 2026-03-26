// HTML sanitizer pipeline — strips Office cruft, normalizes inline CSS.
// See CLIPLI_SPEC.md §5.2 for full specification.

use lol_html::{element, rewrite_str, RewriteStrSettings};
use regex::Regex;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options controlling the HTML cleaning pipeline.
#[derive(Debug, Clone)]
pub struct CleanOptions {
    /// Retain `class` attributes on elements (default: strip them).
    pub keep_classes: bool,
    /// Target application — determines which CSS properties to keep.
    pub target_app: TargetApp,
}

impl Default for CleanOptions {
    fn default() -> Self {
        Self {
            keep_classes: false,
            target_app: TargetApp::Generic,
        }
    }
}

/// The application that will receive the pasted HTML.
/// Determines which CSS properties survive the cleaning pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetApp {
    Excel,
    PowerPoint,
    GoogleSheets,
    Generic,
}

/// Errors produced by the cleaning pipeline.
#[derive(Debug, thiserror::Error)]
pub enum CleanError {
    #[error("HTML rewriter error: {0}")]
    Rewriter(String),
    #[allow(dead_code)]
    #[error("encoding error: {0}")]
    Encoding(String),
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the 10-stage HTML cleaning pipeline and return the cleaned HTML.
pub fn clean(html: &str, opts: &CleanOptions) -> Result<String, CleanError> {
    // Stage 1: encoding normalization (pre-processing, before lol_html).
    // When the caller holds &str it's already valid UTF-8; callers with raw
    // bytes should use `clean_bytes` or `decode_bytes` first.
    let html = html.to_owned();

    // Stage 2b (pre-pass): strip conditional comments before lol_html sees them,
    // because lol_html does not understand `<!--[if ...]>...<![endif]-->` syntax.
    let html = strip_conditional_comments(&html);

    // Stages 2a, 3, 4, 6, 7, 8 — streaming lol_html pass.
    let opts_ref = opts.clone();
    let result = rewrite_str(
        &html,
        RewriteStrSettings {
            element_content_handlers: vec![
                // Stage 2a: strip <meta>, <link>, <style>, <xml> and their subtree.
                element!("meta, link, style, xml", |el| {
                    el.remove();
                    Ok(())
                }),
                // Stages 3 + 4 + 6: normalize style attributes.
                // Strips mso-* properties, normalizes font aliases and colors,
                // and filters by the target-app CSS property allowlist.
                element!("*[style]", |el| {
                    if let Some(style) = el.get_attribute("style") {
                        let cleaned = normalize_css(&style, opts_ref.target_app);
                        if cleaned.trim().is_empty() {
                            el.remove_attribute("style");
                        } else {
                            el.set_attribute("style", &cleaned)
                                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                                    e.into()
                                })?;
                        }
                    }
                    Ok(())
                }),
                // Stage 7: strip class attributes (unless keep_classes is set).
                element!("*[class]", |el| {
                    if !opts_ref.keep_classes {
                        el.remove_attribute("class");
                    }
                    Ok(())
                }),
                // Stage 8: strip id attributes.
                element!("*[id]", |el| {
                    el.remove_attribute("id");
                    Ok(())
                }),
            ],
            ..Default::default()
        },
    )
    .map_err(|e| CleanError::Rewriter(e.to_string()))?;

    // Stage 5: collapse empty span/p/div elements (post-pass; repeat until stable).
    let result = collapse_empty_elements(&result);

    // Stage 9: normalize whitespace in text content outside pre/code blocks.
    let result = normalize_whitespace_outside_pre(&result);

    // Stage 10: basic validation — return an error if non-empty input produced
    // empty output (indicates a catastrophic rewrite failure).
    if result.trim().is_empty() && !html.trim().is_empty() {
        return Err(CleanError::Rewriter(
            "cleaning produced empty output from non-empty input".to_string(),
        ));
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Stage 1 — encoding normalization
// ---------------------------------------------------------------------------

/// Decode raw clipboard bytes to a UTF-8 `String`.
///
/// Handles, in order:
/// - UTF-16 LE with BOM (`FF FE`)
/// - UTF-16 BE with BOM (`FE FF`)
/// - UTF-8 with optional BOM (`EF BB BF`)
/// - Plain UTF-8
/// - Windows-1252 (fallback for any byte sequence that is not valid UTF-8)
///
/// Returns `Err(CleanError::Encoding)` only when UTF-16 surrogate-pair decoding
/// fails; all other encodings are decoded losslessly or with substitution.
#[allow(dead_code)]
pub fn decode_bytes(bytes: &[u8]) -> Result<String, CleanError> {
    // UTF-16 LE BOM: FF FE
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let utf16: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16(&utf16)
            .map_err(|e| CleanError::Encoding(e.to_string()));
    }

    // UTF-16 BE BOM: FE FF
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let utf16: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16(&utf16)
            .map_err(|e| CleanError::Encoding(e.to_string()));
    }

    // UTF-8 BOM: EF BB BF — strip the BOM then validate as UTF-8.
    let stripped = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };

    // Valid UTF-8 — common case.
    if let Ok(s) = std::str::from_utf8(stripped) {
        return Ok(s.to_owned());
    }

    // Fallback: Windows-1252.  Map each byte through the Win-1252 → Unicode table.
    Ok(bytes.iter().map(|&b| windows1252_to_char(b)).collect())
}

/// Entry point for raw pasteboard bytes.
/// Decodes the encoding via [`decode_bytes`] then runs the full 10-stage
/// cleaning pipeline.
#[allow(dead_code)]
pub fn clean_bytes(bytes: &[u8], opts: &CleanOptions) -> Result<String, CleanError> {
    let html = decode_bytes(bytes)?;
    clean(&html, opts)
}

/// Map a Windows-1252 byte to its Unicode character.
#[allow(dead_code)]
fn windows1252_to_char(b: u8) -> char {
    // The 0x80–0x9F range has special Windows-1252 mappings.
    const TABLE: [char; 32] = [
        '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}',
        '\u{2020}', '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}',
        '\u{0152}', '\u{FFFD}', '\u{017D}', '\u{FFFD}', '\u{FFFD}', '\u{2018}',
        '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
        '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{FFFD}',
        '\u{017E}', '\u{0178}',
    ];
    match b {
        0x00..=0x7F => b as char,
        0x80..=0x9F => TABLE[(b - 0x80) as usize],
        _ => b as char,
    }
}

// ---------------------------------------------------------------------------
// Stage 2b — strip conditional comments
// ---------------------------------------------------------------------------

/// Remove `<!--[if ...]>...<![endif]-->` and `<![if ...]>...<![endif]>` blocks.
fn strip_conditional_comments(html: &str) -> String {
    // Standard downlevel-hidden conditional comments: <!--[if ...]>...<![endif]-->
    let re_hidden = Regex::new(r"(?s)<!--\[if[^\]]*\]>.*?<!\[endif\]-->").unwrap();
    // Downlevel-revealed conditional comments: <![if ...]>...<![endif]>
    let re_revealed = Regex::new(r"(?s)<!\[if[^\]]*\]>.*?<!\[endif\]>").unwrap();

    let s = re_hidden.replace_all(html, "");
    let s = re_revealed.replace_all(&s, "");
    s.into_owned()
}

// ---------------------------------------------------------------------------
// Stage 5 — collapse empty elements
// ---------------------------------------------------------------------------

/// Repeatedly remove empty `<span>`, `<p>`, and `<div>` elements until stable.
fn collapse_empty_elements(html: &str) -> String {
    // Rust's regex crate does not support backreferences, so we use three
    // separate patterns — one per tag — and apply them in a combined loop.
    let inner = r"(?:\s|&nbsp;|&#160;)*";
    let re_span = Regex::new(&format!(r"(?i)<span[^>]*>{}</span>", inner)).unwrap();
    let re_p = Regex::new(&format!(r"(?i)<p[^>]*>{}</p>", inner)).unwrap();
    let re_div = Regex::new(&format!(r"(?i)<div[^>]*>{}</div>", inner)).unwrap();
    let mut current = html.to_owned();
    loop {
        let next = re_span.replace_all(&current, "").into_owned();
        let next = re_p.replace_all(&next, "").into_owned();
        let next = re_div.replace_all(&next, "").into_owned();
        if next == current {
            break;
        }
        current = next;
    }
    current
}

// ---------------------------------------------------------------------------
// Stage 9 — normalize whitespace in text content
// ---------------------------------------------------------------------------

/// Collapse runs of spaces/tabs in text content outside `<pre>` and `<code>`
/// blocks.  Newlines are preserved to avoid breaking block-level rendering.
fn normalize_whitespace_outside_pre(html: &str) -> String {
    // Split the HTML on <pre>/<code> island boundaries.  Content inside those
    // tags is preserved verbatim; everything else has horizontal whitespace runs
    // collapsed to a single space.
    let re_pre = Regex::new(r"(?is)(<(?:pre|code)(?:\s[^>]*)?>.*?</(?:pre|code)>)").unwrap();
    let re_ws = Regex::new(r"[ \t]+").unwrap();

    let mut result = String::with_capacity(html.len());
    let mut last_end = 0;

    for mat in re_pre.find_iter(html) {
        // Normalize the segment before this <pre>/<code> block.
        let before = &html[last_end..mat.start()];
        result.push_str(&re_ws.replace_all(before, " "));
        // Preserve the <pre>/<code> block unchanged.
        result.push_str(mat.as_str());
        last_end = mat.end();
    }
    // Normalize the trailing segment after the last <pre>/<code> block.
    let tail = &html[last_end..];
    result.push_str(&re_ws.replace_all(tail, " "));

    result
}

// ---------------------------------------------------------------------------
// CSS helpers (stages 3, 4, 6, 7, 9)
// ---------------------------------------------------------------------------

/// Parse a CSS inline style declaration string into `(name, value)` pairs.
///
/// Declarations are separated by semicolons; each declaration is split on the
/// first colon.  Property names are lower-cased.  Empty or malformed
/// declarations (no colon) are silently skipped.
pub fn parse_css_declarations(style: &str) -> Vec<(String, String)> {
    style
        .split(';')
        .filter_map(|decl| {
            let decl = decl.trim();
            if decl.is_empty() {
                return None;
            }
            let colon = decl.find(':')?;
            let name = decl[..colon].trim().to_ascii_lowercase();
            let value = decl[colon + 1..].trim().to_owned();
            if name.is_empty() {
                return None;
            }
            Some((name, value))
        })
        .collect()
}

/// Serialize `(name, value)` pairs back to a CSS inline style string.
///
/// Declarations are joined with `"; "`.  An empty slice produces an empty
/// string (no trailing semicolon).
pub fn serialize_css_declarations(decls: &[(String, String)]) -> String {
    decls
        .iter()
        .map(|(n, v)| format!("{}: {}", n, v))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Convert `rgb(R, G, B)` notation to `#RRGGBB` hex in the given string.
///
/// The match is case-insensitive and tolerates extra spaces around the
/// channel values.  Non-rgb content is returned unchanged.
pub fn rgb_to_hex(s: &str) -> String {
    let re = Regex::new(r"(?i)rgb\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*\)").unwrap();
    re.replace_all(s, |caps: &regex::Captures| {
        let r: u8 = caps[1].parse().unwrap_or(0);
        let g: u8 = caps[2].parse().unwrap_or(0);
        let b: u8 = caps[3].parse().unwrap_or(0);
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    })
    .into_owned()
}

/// Return the list of CSS property names preserved for the given target app.
///
/// The table mirrors §5.2 of CLIPLI_SPEC.md.
pub fn allowed_css_properties(target: TargetApp) -> &'static [&'static str] {
    // Universal 8 (shared by all targets)
    const UNIVERSAL: &[&str] = &[
        "font-family",
        "font-size",
        "font-weight",
        "font-style",
        "color",
        "background-color",
        "text-align",
        "text-decoration",
    ];
    const EXCEL: &[&str] = &[
        "font-family",
        "font-size",
        "font-weight",
        "font-style",
        "color",
        "background-color",
        "text-align",
        "text-decoration",
        "border",
        "border-collapse",
        "padding",
        "width",
        "height",
        "vertical-align",
        "white-space",
    ];
    const SHEETS: &[&str] = &[
        "font-family",
        "font-size",
        "font-weight",
        "font-style",
        "color",
        "background-color",
        "text-align",
        "text-decoration",
        "border",
        "border-collapse",
        "padding",
        "width",
        "height",
        "vertical-align",
    ];
    const GENERIC: &[&str] = &[
        "font-family",
        "font-size",
        "font-weight",
        "font-style",
        "color",
        "background-color",
        "text-align",
        "text-decoration",
        "border",
        "border-collapse",
        "padding",
        "width",
        "height",
        "vertical-align",
        "white-space",
    ];

    match target {
        TargetApp::PowerPoint => UNIVERSAL,
        TargetApp::Excel => EXCEL,
        TargetApp::GoogleSheets => SHEETS,
        TargetApp::Generic => GENERIC,
    }
}

/// Normalize a single CSS inline-style declaration list string.
///
/// Pipeline applied:
/// 1. Strip `mso-*`, `panose-1`, and `tab-stops` properties (stage 3).
/// 2. Normalize font aliases (`+mj-lt` → `Calibri`, etc.) (stage 4).
/// 3. Convert `rgb()` to hex; replace `windowtext` with `#000000` (stage 6).
/// 4. Filter to the CSS property allowlist for `target` (stage 7).
pub fn normalize_css(style: &str, target: TargetApp) -> String {
    let decls = parse_css_declarations(style);
    let allowed = allowed_css_properties(target);

    let filtered: Vec<(String, String)> = decls
        .into_iter()
        .filter_map(|(name, value)| {
            // Stage 3: strip mso-* and other Office-only declarations.
            if name.starts_with("mso-") || name == "panose-1" || name == "tab-stops" {
                return None;
            }

            // Stage 7: allowlist check.
            // Allow exact matches and `border-*` sub-property variants when
            // the base `border` property is in the allowlist for this target.
            let allowed_by_list = allowed.contains(&name.as_str())
                || (name.starts_with("border-") && allowed.contains(&"border"));

            if !allowed_by_list {
                return None;
            }

            // Stage 4: normalize font aliases in font-family / shorthand font.
            let value = if name == "font-family" || name == "font" {
                normalize_font_aliases(&value)
            } else {
                value
            };

            // Stage 6: normalize colors.
            let value = normalize_colors(&value);

            Some((name, value))
        })
        .collect();

    serialize_css_declarations(&filtered)
}

/// Public alias kept for callers that want to filter CSS explicitly.
#[allow(dead_code)]
pub fn filter_css_for_target(style: &str, target: TargetApp) -> String {
    normalize_css(style, target)
}

// ---------------------------------------------------------------------------
// Stage 3/4 helpers
// ---------------------------------------------------------------------------

/// Replace Office font aliases with web-safe equivalents.
///
/// Maps:
/// - `+mj-lt`, `+mj-ea`, `+mn-lt`, `+mn-ea`, `+mj-cs`, `+mn-cs` → `Calibri`
/// - `Wingdings` (and variants), `Symbol` → `Arial`
fn normalize_font_aliases(val: &str) -> String {
    // Office theme font names appear both quoted (inside CSS font-family) and
    // unquoted; handle both forms.
    let s = val
        .replace(r#""+mj-lt""#, "Calibri")
        .replace(r#""+mj-ea""#, "Calibri")
        .replace(r#""+mn-lt""#, "Calibri")
        .replace(r#""+mn-ea""#, "Calibri")
        .replace(r#""+mj-cs""#, "Calibri")
        .replace(r#""+mn-cs""#, "Calibri")
        .replace("+mj-lt", "Calibri")
        .replace("+mj-ea", "Calibri")
        .replace("+mn-lt", "Calibri")
        .replace("+mn-ea", "Calibri")
        .replace("+mj-cs", "Calibri")
        .replace("+mn-cs", "Calibri");

    // Replace symbol / dingbat fonts with Arial.
    let re_sym = Regex::new(r"(?i)\b(Wingdings\d*|Symbol)\b").unwrap();
    re_sym.replace_all(&s, "Arial").into_owned()
}

// ---------------------------------------------------------------------------
// Stage 6 helper
// ---------------------------------------------------------------------------

/// Convert `rgb(R, G, B)` → `#RRGGBB` and replace named color keywords with hex.
///
/// Handles (case-insensitive):
/// - `windowtext` → `#000000`
/// - `black`      → `#000000`
/// - `white`      → `#FFFFFF`
///
/// Delegates `rgb()` conversion to [`rgb_to_hex`].
fn normalize_colors(val: &str) -> String {
    // Replace named color keywords (case-insensitive, word-boundary anchored so
    // that substrings like "blackout" are not affected).
    let re_wt = Regex::new(r"(?i)\bwindowtext\b").unwrap();
    let re_black = Regex::new(r"(?i)\bblack\b").unwrap();
    let re_white = Regex::new(r"(?i)\bwhite\b").unwrap();

    let s = re_wt.replace_all(val, "#000000").into_owned();
    let s = re_black.replace_all(&s, "#000000").into_owned();
    let s = re_white.replace_all(&s, "#FFFFFF").into_owned();

    // Convert rgb(...) to hex via the public helper.
    rgb_to_hex(&s)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> CleanOptions {
        CleanOptions::default()
    }

    fn opts_target(target: TargetApp) -> CleanOptions {
        CleanOptions {
            keep_classes: false,
            target_app: target,
        }
    }

    // 1. mso-* properties are stripped from style attributes.
    #[test]
    fn strips_mso_properties() {
        let html = r#"<p style="mso-margin-top-alt:auto; color: red; mso-line-height-rule:exactly">Hello</p>"#;
        let result = clean(html, &opts()).unwrap();
        assert!(
            !result.contains("mso-"),
            "mso-* properties should be stripped, got: {result}"
        );
        assert!(
            result.contains("color: red"),
            "non-mso properties should be kept, got: {result}"
        );
    }

    // 2. rgb() → hex conversion.
    #[test]
    fn rgb_color_becomes_hex() {
        let html = r#"<span style="color: rgb(255, 0, 0)">text</span>"#;
        let result = clean(html, &opts()).unwrap();
        assert!(
            result.contains("#FF0000"),
            "rgb(255,0,0) should become #FF0000, got: {result}"
        );
    }

    // 3. windowtext → #000000.
    #[test]
    fn windowtext_to_black() {
        let html = r#"<td style="border: .5pt solid windowtext">cell</td>"#;
        let result = clean(html, &opts()).unwrap();
        assert!(
            result.contains("#000000"),
            "windowtext should become #000000, got: {result}"
        );
        assert!(
            !result.contains("windowtext"),
            "windowtext keyword should be gone, got: {result}"
        );
    }

    // 4. class attributes stripped by default; kept with keep_classes.
    #[test]
    fn class_stripped_by_default() {
        let html = r#"<p class="MsoNormal">text</p>"#;
        let result = clean(html, &opts()).unwrap();
        assert!(
            !result.contains("class="),
            "class attribute should be stripped by default, got: {result}"
        );
    }

    #[test]
    fn class_kept_when_requested() {
        let html = r#"<p class="MsoNormal">text</p>"#;
        let keep_opts = CleanOptions {
            keep_classes: true,
            target_app: TargetApp::Generic,
        };
        let result = clean(html, &keep_opts).unwrap();
        assert!(
            result.contains(r#"class="MsoNormal""#),
            "class attribute should be kept with keep_classes=true, got: {result}"
        );
    }

    // 5. <meta> tags are removed.
    #[test]
    fn meta_tags_removed() {
        let html = r#"<html><head><meta http-equiv="Content-Type" content="text/html; charset=utf-8"><meta name="Generator" content="Microsoft Excel 16"></head><body><p>Hello</p></body></html>"#;
        let result = clean(html, &opts()).unwrap();
        assert!(
            !result.contains("<meta"),
            "<meta> tags should be removed, got: {result}"
        );
        assert!(
            result.contains("Hello"),
            "body content should be kept, got: {result}"
        );
    }

    // 6. Empty <span></span> elements are collapsed.
    #[test]
    fn empty_spans_collapsed() {
        let html = "<p>Hello<span></span> World<span>  </span></p>";
        let result = clean(html, &opts()).unwrap();
        assert!(
            !result.contains("<span"),
            "empty spans should be collapsed, got: {result}"
        );
        assert!(
            result.contains("Hello"),
            "text content should be preserved, got: {result}"
        );
    }

    // 7. PowerPoint target strips `border`; Generic keeps it.
    #[test]
    fn ppt_target_strips_border() {
        let html = r#"<td style="border: 1px solid #000; color: red">cell</td>"#;

        let ppt_result = clean(html, &opts_target(TargetApp::PowerPoint)).unwrap();
        assert!(
            !ppt_result.contains("border"),
            "border should be stripped for PowerPoint, got: {ppt_result}"
        );
        assert!(
            ppt_result.contains("color: red"),
            "color should be kept for PowerPoint, got: {ppt_result}"
        );
    }

    #[test]
    fn generic_target_keeps_border() {
        let html = r#"<td style="border: 1px solid #000; color: red">cell</td>"#;
        let generic_result = clean(html, &opts_target(TargetApp::Generic)).unwrap();
        assert!(
            generic_result.contains("border"),
            "border should be kept for Generic, got: {generic_result}"
        );
    }

    // 8. Empty input returns empty string (no error).
    #[test]
    fn empty_input_returns_empty() {
        let result = clean("", &opts()).unwrap();
        assert!(result.is_empty(), "empty input should produce empty output, got: {result}");
    }

    // 9. Plain text (no HTML tags) passes through.
    #[test]
    fn plain_text_passthrough() {
        let text = "Hello, world! No HTML here.";
        let result = clean(text, &opts()).unwrap();
        assert!(
            result.contains("Hello, world!"),
            "plain text should pass through unchanged, got: {result}"
        );
    }

    // Additional: font aliases are normalized.
    #[test]
    fn font_aliases_normalized() {
        let html = r#"<span style='font-family:"+mj-lt"'>text</span>"#;
        let result = clean(html, &opts()).unwrap();
        assert!(
            result.contains("Calibri"),
            "+mj-lt should be replaced with Calibri, got: {result}"
        );
        assert!(
            !result.contains("+mj-lt"),
            "+mj-lt alias should be gone, got: {result}"
        );
    }

    // Additional: <style> blocks are removed.
    #[test]
    fn style_blocks_removed() {
        let html = "<html><head><style>p { color: red; }</style></head><body><p>Text</p></body></html>";
        let result = clean(html, &opts()).unwrap();
        assert!(
            !result.contains("<style"),
            "<style> blocks should be removed, got: {result}"
        );
        assert!(
            result.contains("Text"),
            "body content should be preserved, got: {result}"
        );
    }

    // Additional: conditional comments are stripped.
    #[test]
    fn conditional_comments_stripped() {
        let html = r#"<body><!--[if gte mso 9]><xml><o:OfficeDocumentSettings></o:OfficeDocumentSettings></xml><![endif]--><p>Content</p></body>"#;
        let result = clean(html, &opts()).unwrap();
        assert!(
            !result.contains("<!--[if"),
            "conditional comments should be stripped, got: {result}"
        );
        assert!(
            result.contains("Content"),
            "body content should be preserved, got: {result}"
        );
    }

    // Additional: normalize_colors unit tests.
    #[test]
    fn normalize_colors_rgb_various() {
        assert_eq!(normalize_colors("rgb(0, 0, 0)"), "#000000");
        assert_eq!(normalize_colors("rgb(255, 255, 255)"), "#FFFFFF");
        assert_eq!(normalize_colors("rgb(0, 128, 0)"), "#008000");
    }

    #[test]
    fn normalize_colors_windowtext_case_insensitive() {
        assert_eq!(normalize_colors("WindowText"), "#000000");
        assert_eq!(normalize_colors("WINDOWTEXT"), "#000000");
    }

    // Additional: panose-1 is stripped.
    #[test]
    fn panose_stripped() {
        let html = r#"<span style="panose-1:2 4 5 3 5 4 4 2 2 3; font-weight: bold">text</span>"#;
        let result = clean(html, &opts()).unwrap();
        assert!(
            !result.contains("panose-1"),
            "panose-1 should be stripped, got: {result}"
        );
        assert!(
            result.contains("font-weight: bold"),
            "font-weight should be kept, got: {result}"
        );
    }

    // Additional: id attributes are stripped.
    #[test]
    fn id_attributes_stripped() {
        let html = r#"<div id="slide1"><p>Content</p></div>"#;
        let result = clean(html, &opts()).unwrap();
        assert!(
            !result.contains("id="),
            "id attributes should be stripped, got: {result}"
        );
        assert!(
            result.contains("Content"),
            "content should be preserved, got: {result}"
        );
    }

    // Additional: Excel target keeps white-space; Sheets does not.
    #[test]
    fn excel_keeps_whitespace_sheets_strips_it() {
        let html = r#"<td style="white-space: nowrap; color: blue">cell</td>"#;

        let excel_result = clean(html, &opts_target(TargetApp::Excel)).unwrap();
        assert!(
            excel_result.contains("white-space"),
            "white-space should be kept for Excel, got: {excel_result}"
        );

        let sheets_result = clean(html, &opts_target(TargetApp::GoogleSheets)).unwrap();
        assert!(
            !sheets_result.contains("white-space"),
            "white-space should be stripped for GoogleSheets, got: {sheets_result}"
        );
    }

    // Additional: border kept for Excel; stripped for PowerPoint.
    #[test]
    fn border_kept_for_excel_stripped_for_ppt() {
        let html = r#"<td style="border: .5pt solid #000000; font-size: 11pt">Cell</td>"#;

        let excel = clean(html, &opts_target(TargetApp::Excel)).unwrap();
        assert!(
            excel.contains("border"),
            "border should be kept for Excel, got: {excel}"
        );

        let ppt = clean(html, &opts_target(TargetApp::PowerPoint)).unwrap();
        assert!(
            !ppt.contains("border"),
            "border should be stripped for PowerPoint, got: {ppt}"
        );
    }

    // Additional: Symbol font replaced with Arial.
    #[test]
    fn symbol_font_replaced_with_arial() {
        let html = r#"<span style="font-family: Symbol; font-size: 20pt">·</span>"#;
        let result = clean(html, &opts()).unwrap();
        assert!(
            !result.contains("Symbol"),
            "Symbol font should be replaced, got: {result}"
        );
        assert!(
            result.contains("Arial"),
            "Symbol should become Arial, got: {result}"
        );
    }

    // Additional: decode_bytes UTF-8 BOM is stripped.
    #[test]
    fn decode_bytes_strips_utf8_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(b"<p>Hello</p>");
        let result = decode_bytes(&bytes).unwrap();
        assert_eq!(result, "<p>Hello</p>");
    }

    // Additional: decode_bytes handles UTF-16 LE.
    #[test]
    fn decode_bytes_utf16_le() {
        let text = "Hi";
        let mut bytes = vec![0xFF, 0xFE]; // LE BOM
        for c in text.encode_utf16() {
            bytes.extend_from_slice(&c.to_le_bytes());
        }
        let result = decode_bytes(&bytes).unwrap();
        assert_eq!(result, "Hi");
    }

    // Additional: public helpers work correctly.
    #[test]
    fn rgb_to_hex_helper_direct() {
        assert_eq!(super::rgb_to_hex("rgb(255, 0, 0)"), "#FF0000");
        assert_eq!(super::rgb_to_hex("rgb(0, 128, 255)"), "#0080FF");
        assert_eq!(super::rgb_to_hex("no color here"), "no color here");
    }

    #[test]
    fn parse_serialize_css_round_trip() {
        let style = "font-size: 11pt; color: red; font-weight: bold";
        let decls = parse_css_declarations(style);
        assert_eq!(decls.len(), 3);
        assert_eq!(decls[0], ("font-size".to_string(), "11pt".to_string()));
        let serialized = serialize_css_declarations(&decls);
        assert!(serialized.contains("font-size: 11pt"));
        assert!(serialized.contains("color: red"));
    }

    // 9. Snapshot-style test: representative Office HTML cruft is cleaned properly.
    #[test]
    fn office_html_snapshot_clean() {
        let input = r#"<html>
<head>
<meta http-equiv="Content-Type" content="text/html; charset=utf-8">
<meta name="Generator" content="Microsoft Excel 16">
<style>
.xl65 { mso-font-charset:0; color:#FFFFFF; font-size:11.0pt; }
</style>
</head>
<body>
<!--[if gte mso 9]><xml>
 <x:ExcelWorkbook/>
</xml><![endif]-->
<table style="border-collapse:collapse;mso-border-alt:solid windowtext .5pt">
<tr>
  <td class="xl65"
      style="mso-font-charset:0;color:rgb(255,255,255);font-family:&quot;+mj-lt&quot;;font-size:11.0pt;font-weight:700;border:.5pt solid windowtext;background:#1F497D">
    Header
  </td>
  <td style="mso-font-charset:0;color:black;font-size:11.0pt;font-weight:400;border:.5pt solid windowtext">
    Value
  </td>
</tr>
</table>
</body>
</html>"#;

        let result = clean(
            input,
            &CleanOptions {
                keep_classes: false,
                target_app: TargetApp::Excel,
            },
        )
        .unwrap();

        // Must NOT contain mso-* properties.
        assert!(!result.contains("mso-"), "mso-* properties must be stripped, got: {result}");

        // Must NOT contain class or id attributes.
        assert!(!result.contains("class="), "class attrs must be stripped, got: {result}");

        // Must NOT contain conditional comments.
        assert!(!result.contains("<!--[if"), "conditional comments must be stripped");

        // Must NOT contain <meta> or <style> elements.
        assert!(!result.contains("<meta"), "<meta> must be stripped");
        assert!(!result.contains("<style"), "<style> must be stripped");

        // Must NOT contain rgb() or windowtext color notations.
        assert!(!result.contains("rgb("), "rgb() must be converted to hex");
        assert!(!result.contains("windowtext"), "windowtext must be replaced");

        // Must NOT contain Office font alias.
        assert!(!result.contains("+mj-lt"), "font alias must be replaced");

        // Allowed CSS properties must be present where they were set.
        assert!(result.contains("font-size"), "font-size should be preserved");
        assert!(result.contains("font-weight"), "font-weight should be preserved");
        assert!(result.contains("border"), "border should be preserved for Excel");

        // Content must be preserved.
        assert!(result.contains("Header"), "cell content must be preserved");
        assert!(result.contains("Value"), "cell content must be preserved");
    }
}
