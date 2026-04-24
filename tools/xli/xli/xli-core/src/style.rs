use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Workbook number format specification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NumberFormat {
    pub pattern: String,
}

/// Cell fill styling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FillSpec {
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

/// Font styling.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FontSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
}

/// Aggregate cell style definition used by formatting commands.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StyleSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(rename = "font_color", skip_serializing_if = "Option::is_none")]
    pub font_color: Option<String>,
    #[serde(rename = "fill", skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(rename = "number_format", skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub horizontal_align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<String>,
}

/// Resolve an agent-friendly number format alias into an Excel format code.
///
/// Unknown formats are treated as literal Excel format strings so existing
/// callers can continue to pass custom number formats directly.
pub fn resolve_number_format(format: &str) -> String {
    match normalize_format_alias(format).as_str() {
        "currency" => "$#,##0;[Red]($#,##0)".to_string(),
        "currency_2dp" => "$#,##0.00;[Red]($#,##0.00)".to_string(),
        "accounting" => "_(* #,##0_);_(* (#,##0);_(* \"-\"??_);_(@_)".to_string(),
        "accounting_2dp" => "_(* #,##0.00_);_(* (#,##0.00);_(* \"-\"??_);_(@_)".to_string(),
        "percent" => "0.00%".to_string(),
        "percent_int" => "0%".to_string(),
        "percent_1dp" => "0.0%".to_string(),
        "integer" => "#,##0".to_string(),
        "standard" => "#,##0.00".to_string(),
        "text" => "@".to_string(),
        "date_iso" => "yyyy-mm-dd".to_string(),
        "datetime_iso" => "yyyy-mm-dd hh:mm".to_string(),
        _ => format.to_string(),
    }
}

fn normalize_format_alias(format: &str) -> String {
    format.trim().to_ascii_lowercase().replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::resolve_number_format;

    #[test]
    fn resolves_known_number_format_aliases() {
        assert_eq!(resolve_number_format("currency"), "$#,##0;[Red]($#,##0)");
        assert_eq!(resolve_number_format("percent-int"), "0%");
        assert_eq!(resolve_number_format("datetime_iso"), "yyyy-mm-dd hh:mm");
    }

    #[test]
    fn leaves_custom_number_formats_unchanged() {
        let custom = "$#,##0;($#,##0);\"-\"";
        assert_eq!(resolve_number_format(custom), custom);
    }
}
