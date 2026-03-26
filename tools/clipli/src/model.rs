// Data model — all shared types for clipli.
// See CLIPLI_SPEC.md §3 for full specification.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// §3.1  Pasteboard Types
// ---------------------------------------------------------------------------

/// Recognized pasteboard UTI types, in priority order for capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PbType {
    Html,       // public.html
    Rtf,        // public.rtf
    PlainText,  // public.utf8-plain-text
    Png,        // public.png
    Tiff,       // public.tiff
    Pdf,        // com.adobe.pdf
    Unknown,    // anything else (logged but not processed)
}

impl PbType {
    /// Return the canonical UTI string for this type.
    pub fn uti(&self) -> &'static str {
        match self {
            Self::Html      => "public.html",
            Self::Rtf       => "public.rtf",
            Self::PlainText => "public.utf8-plain-text",
            Self::Png       => "public.png",
            Self::Tiff      => "public.tiff",
            Self::Pdf       => "com.adobe.pdf",
            Self::Unknown   => "unknown",
        }
    }

    /// Map a UTI string to the corresponding variant.
    /// Returns `PbType::Unknown` for unrecognised strings.
    pub fn from_uti(s: &str) -> Self {
        match s {
            "public.html"            => Self::Html,
            "public.rtf"             => Self::Rtf,
            "public.utf8-plain-text" => Self::PlainText,
            "public.png"             => Self::Png,
            "public.tiff"            => Self::Tiff,
            "com.adobe.pdf"          => Self::Pdf,
            _                        => Self::Unknown,
        }
    }
}

/// Raw pasteboard snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbSnapshot {
    pub types: Vec<PbTypeEntry>,
    pub captured_at: chrono::DateTime<chrono::Utc>,
    /// From NSPasteboard owner, if available.
    pub source_app: Option<String>,
}

/// One type slot from a pasteboard snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbTypeEntry {
    pub pb_type: PbType,
    pub uti: String,
    pub size_bytes: usize,
    pub data: Vec<u8>,
}

impl PbTypeEntry {
    /// Return the raw bytes as a UTF-8 `&str` if this entry holds text data.
    ///
    /// Returns `None` when the data is not valid UTF-8 or the type is not a
    /// text-bearing type (`Html`, `Rtf`, `PlainText`).
    #[allow(dead_code)]
    pub fn as_utf8_str(&self) -> Option<&str> {
        match self.pb_type {
            PbType::Html | PbType::Rtf | PbType::PlainText => {
                std::str::from_utf8(&self.data).ok()
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// §3.2  Template Metadata
// ---------------------------------------------------------------------------

/// Stored alongside every captured template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMeta {
    pub name: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub source_app: Option<String>,
    /// UTIs present at capture time.
    pub source_pb_types: Vec<String>,
    /// Was variable extraction performed?
    #[serde(default)]
    pub templatized: bool,
    /// Extracted template variables.
    #[serde(default)]
    pub variables: Vec<TemplateVariable>,
    /// User-defined tags for search.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A single variable extracted from a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    pub var_type: VarType,
    pub default_value: Option<serde_json::Value>,
    pub description: Option<String>,
}

/// Semantic type of a template variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VarType {
    String,
    Number,
    Currency,
    Percentage,
    Date,
    Boolean,
    List,
}

// ---------------------------------------------------------------------------
// §3.3  Table Model
// ---------------------------------------------------------------------------

/// Agent-friendly table input format for `clipli paste --from-table`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInput {
    pub headers: Option<Vec<Cell>>,
    pub rows: Vec<Vec<Cell>>,
    pub style: Option<TableStyle>,
}

/// A single table cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub value: String,
    #[serde(default)]
    pub style: CellStyle,
}

/// Per-cell formatting options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CellStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size_pt: Option<f32>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    /// Foreground colour as a hex string, e.g. `"#1A3E6F"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alignment: Option<Align>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<BorderStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colspan: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rowspan: Option<u32>,
    /// Excel number format string, e.g. "currency", "percent", "integer", "text".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    /// URL for hyperlinked cells. Renders as <a href> with styled span.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Enable word wrapping (white-space:normal). Default: nowrap.
    #[serde(default)]
    pub wrap: bool,
    /// Excel formula (e.g. "=SUM(B2:B5)"). Emitted as x:fmla attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
}

/// Horizontal text alignment inside a cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Border specification for a cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_px: Option<f32>,
    /// One of `"solid"`, `"dashed"`, or `"dotted"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}

/// Table-level style defaults applied to every cell unless overridden.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_font_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_font_size_pt: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stripe_even_bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_collapse: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outer_border: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_border: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_border: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- PbType ---

    #[test]
    fn pb_type_uti_round_trip() {
        let variants = [
            PbType::Html,
            PbType::Rtf,
            PbType::PlainText,
            PbType::Png,
            PbType::Tiff,
            PbType::Pdf,
            PbType::Unknown,
        ];
        for v in variants {
            let uti = v.uti();
            let recovered = PbType::from_uti(uti);
            assert_eq!(v, recovered, "round-trip failed for {:?}", v);
        }
    }

    #[test]
    fn pb_type_from_uti_unknown() {
        assert_eq!(PbType::from_uti("com.example.custom"), PbType::Unknown);
    }

    #[test]
    fn pb_type_serde_round_trip() {
        let original = PbType::PlainText;
        let json_str = serde_json::to_string(&original).unwrap();
        let recovered: PbType = serde_json::from_str(&json_str).unwrap();
        assert_eq!(original, recovered);
    }

    // --- PbSnapshot ---

    #[test]
    fn pb_snapshot_serde_round_trip() {
        let now = chrono::Utc::now();
        let snapshot = PbSnapshot {
            types: vec![PbTypeEntry {
                pb_type: PbType::Html,
                uti: "public.html".to_string(),
                size_bytes: 5,
                data: b"hello".to_vec(),
            }],
            captured_at: now,
            source_app: Some("com.apple.Safari".to_string()),
        };

        let json_str = serde_json::to_string(&snapshot).unwrap();
        let recovered: PbSnapshot = serde_json::from_str(&json_str).unwrap();

        assert_eq!(recovered.source_app, snapshot.source_app);
        assert_eq!(recovered.types.len(), 1);
        assert_eq!(recovered.types[0].size_bytes, 5);
        assert_eq!(recovered.types[0].data, b"hello");
    }

    #[test]
    fn pb_snapshot_source_app_none() {
        let value = json!({
            "types": [],
            "captured_at": "2025-01-01T00:00:00Z",
            "source_app": null
        });
        let snap: PbSnapshot = serde_json::from_value(value).unwrap();
        assert!(snap.source_app.is_none());
        assert!(snap.types.is_empty());
    }

    // --- PbTypeEntry::as_utf8_str ---

    #[test]
    fn as_utf8_str_text_types() {
        for pb_type in [PbType::Html, PbType::Rtf, PbType::PlainText] {
            let entry = PbTypeEntry {
                pb_type,
                uti: pb_type.uti().to_string(),
                size_bytes: 4,
                data: b"test".to_vec(),
            };
            assert_eq!(entry.as_utf8_str(), Some("test"));
        }
    }

    #[test]
    fn as_utf8_str_binary_types_return_none() {
        for pb_type in [PbType::Png, PbType::Tiff, PbType::Pdf, PbType::Unknown] {
            let entry = PbTypeEntry {
                pb_type,
                uti: pb_type.uti().to_string(),
                size_bytes: 4,
                data: b"test".to_vec(),
            };
            assert!(entry.as_utf8_str().is_none());
        }
    }

    #[test]
    fn as_utf8_str_invalid_utf8_returns_none() {
        let entry = PbTypeEntry {
            pb_type: PbType::Html,
            uti: "public.html".to_string(),
            size_bytes: 2,
            data: vec![0xFF, 0xFE], // not valid UTF-8
        };
        assert!(entry.as_utf8_str().is_none());
    }

    // --- TemplateMeta ---

    #[test]
    fn template_meta_serde_round_trip() {
        let value = json!({
            "name": "quarterly_report",
            "description": "Q1 earnings table",
            "created_at": "2025-03-01T12:00:00Z",
            "updated_at": "2025-03-15T09:30:00Z",
            "source_app": "com.microsoft.Excel",
            "source_pb_types": ["public.html", "public.rtf"],
            "templatized": true,
            "variables": [
                {
                    "name": "revenue",
                    "var_type": "currency",
                    "default_value": null,
                    "description": "Total Q1 revenue"
                }
            ],
            "tags": ["finance", "quarterly"]
        });

        let meta: TemplateMeta = serde_json::from_value(value).unwrap();
        assert_eq!(meta.name, "quarterly_report");
        assert!(meta.templatized);
        assert_eq!(meta.variables.len(), 1);
        assert_eq!(meta.variables[0].name, "revenue");
        assert_eq!(meta.tags.len(), 2);

        // Re-serialise and deserialise once more
        let json_str = serde_json::to_string(&meta).unwrap();
        let recovered: TemplateMeta = serde_json::from_str(&json_str).unwrap();
        assert_eq!(recovered.name, meta.name);
    }

    #[test]
    fn template_meta_defaults_for_optional_vecs() {
        // `templatized`, `variables`, and `tags` should all default when absent.
        let value = json!({
            "name": "minimal",
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-01T00:00:00Z",
            "source_pb_types": []
        });
        let meta: TemplateMeta = serde_json::from_value(value).unwrap();
        assert!(!meta.templatized);
        assert!(meta.variables.is_empty());
        assert!(meta.tags.is_empty());
    }

    // --- TableInput ---

    #[test]
    fn table_input_serde_round_trip() {
        let value = json!({
            "headers": [
                {"value": "Product", "style": {"bold": true}},
                {"value": "Price",   "style": {"bold": true, "alignment": "right"}}
            ],
            "rows": [
                [
                    {"value": "Widget A", "style": {}},
                    {"value": "$9.99",    "style": {"alignment": "right", "fg_color": "#2E7D32"}}
                ]
            ],
            "style": {
                "header_bg": "#1A3E6F",
                "header_fg": "#FFFFFF",
                "border_collapse": true
            }
        });

        let table: TableInput = serde_json::from_value(value).unwrap();

        let headers = table.headers.as_ref().unwrap();
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0].value, "Product");
        assert!(headers[0].style.bold);

        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0][0].value, "Widget A");

        let style = table.style.as_ref().unwrap();
        assert_eq!(style.border_collapse, Some(true));

        // round-trip
        let json_str = serde_json::to_string(&table).unwrap();
        let recovered: TableInput = serde_json::from_str(&json_str).unwrap();
        assert_eq!(recovered.rows[0][1].value, "$9.99");
    }

    #[test]
    fn table_input_no_headers_no_style() {
        let value = json!({
            "rows": [[{"value": "only cell"}]]
        });
        let table: TableInput = serde_json::from_value(value).unwrap();
        assert!(table.headers.is_none());
        assert!(table.style.is_none());
        assert_eq!(table.rows[0][0].value, "only cell");
    }

    // --- CellStyle ---

    #[test]
    fn cell_style_default_values() {
        let style = CellStyle::default();
        assert!(!style.bold);
        assert!(!style.italic);
        assert!(style.font_family.is_none());
        assert!(style.font_size_pt.is_none());
        assert!(style.fg_color.is_none());
        assert!(style.bg_color.is_none());
        assert!(style.alignment.is_none());
        assert!(style.border.is_none());
        assert!(style.colspan.is_none());
        assert!(style.rowspan.is_none());
    }

    #[test]
    fn cell_style_serde_round_trip() {
        let value = json!({
            "font_family": "Calibri",
            "font_size_pt": 11.0,
            "bold": true,
            "italic": false,
            "fg_color": "#000000",
            "bg_color": "#FAFAFA",
            "alignment": "center",
            "border": {
                "color": "#CCCCCC",
                "width_px": 1.0,
                "style": "solid"
            },
            "colspan": 2,
            "rowspan": 1
        });

        let style: CellStyle = serde_json::from_value(value).unwrap();
        assert_eq!(style.font_family.as_deref(), Some("Calibri"));
        assert!(style.bold);
        assert_eq!(style.colspan, Some(2));

        let border = style.border.as_ref().unwrap();
        assert_eq!(border.style.as_deref(), Some("solid"));

        let json_str = serde_json::to_string(&style).unwrap();
        let recovered: CellStyle = serde_json::from_str(&json_str).unwrap();
        assert_eq!(recovered.fg_color.as_deref(), Some("#000000"));
    }

    #[test]
    fn cell_style_bools_default_to_false_when_absent() {
        let value = json!({"font_family": "Arial"});
        let style: CellStyle = serde_json::from_value(value).unwrap();
        assert!(!style.bold);
        assert!(!style.italic);
    }

    // --- Align ---

    #[test]
    fn align_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&Align::Left).unwrap(),
            "\"left\""
        );
        assert_eq!(
            serde_json::to_string(&Align::Center).unwrap(),
            "\"center\""
        );
        assert_eq!(
            serde_json::to_string(&Align::Right).unwrap(),
            "\"right\""
        );

        let recovered: Align = serde_json::from_str("\"center\"").unwrap();
        assert!(matches!(recovered, Align::Center));
    }

    // --- VarType ---

    #[test]
    fn var_type_serde_snake_case() {
        let variants = [
            (VarType::String,     "\"string\""),
            (VarType::Number,     "\"number\""),
            (VarType::Currency,   "\"currency\""),
            (VarType::Percentage, "\"percentage\""),
            (VarType::Date,       "\"date\""),
            (VarType::Boolean,    "\"boolean\""),
            (VarType::List,       "\"list\""),
        ];
        for (v, expected) in variants {
            assert_eq!(serde_json::to_string(&v).unwrap(), expected);
        }
    }
}
