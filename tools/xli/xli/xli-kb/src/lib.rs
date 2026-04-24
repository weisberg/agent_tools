#![forbid(unsafe_code)]

//! Built-in knowledge-base templates for repeatable xli operations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use xli_core::{BatchOp, StyleSpec, XliError};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TemplateMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub parameters: Vec<TemplateParameter>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TemplateParameter {
    pub name: String,
    pub required: bool,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TemplatePreview {
    pub template: TemplateMetadata,
    pub ops: Vec<BatchOp>,
}

pub fn list_templates() -> Vec<TemplateMetadata> {
    vec![basic_table_format_metadata()]
}

pub fn get_template(name: &str) -> Result<TemplateMetadata, XliError> {
    list_templates()
        .into_iter()
        .find(|template| template.name == name)
        .ok_or_else(|| XliError::TemplateNotFound {
            template: name.to_string(),
        })
}

pub fn validate_template(name: &str, params: &BTreeMap<String, String>) -> Result<(), XliError> {
    let template = get_template(name)?;
    for parameter in &template.parameters {
        if parameter.required && !params.contains_key(&parameter.name) {
            return Err(XliError::TemplateParamMissing {
                parameter: parameter.name.clone(),
            });
        }
    }
    Ok(())
}

pub fn preview_template(
    name: &str,
    params: &BTreeMap<String, String>,
) -> Result<TemplatePreview, XliError> {
    validate_template(name, params)?;
    let template = get_template(name)?;
    let ops = expand_template(name, params)?;
    Ok(TemplatePreview { template, ops })
}

pub fn expand_template(
    name: &str,
    params: &BTreeMap<String, String>,
) -> Result<Vec<BatchOp>, XliError> {
    match name {
        "basic-table-format" => expand_basic_table_format(params),
        _ => Err(XliError::TemplateNotFound {
            template: name.to_string(),
        }),
    }
}

fn basic_table_format_metadata() -> TemplateMetadata {
    TemplateMetadata {
        name: "basic-table-format".to_string(),
        version: "0.1.0".to_string(),
        description: "Apply common header and body formatting to an existing table range."
            .to_string(),
        parameters: vec![
            TemplateParameter {
                name: "range".to_string(),
                required: true,
                description: "Full table range, for example Sheet1!A1:D20.".to_string(),
            },
            TemplateParameter {
                name: "header_range".to_string(),
                required: false,
                description: "Header range. Defaults to the first row of range.".to_string(),
            },
            TemplateParameter {
                name: "number_format".to_string(),
                required: false,
                description: "Optional number format alias or Excel format for the body range."
                    .to_string(),
            },
        ],
    }
}

fn expand_basic_table_format(params: &BTreeMap<String, String>) -> Result<Vec<BatchOp>, XliError> {
    validate_template("basic-table-format", params)?;
    let range = required_param(params, "range")?;
    let header_range = params
        .get("header_range")
        .cloned()
        .unwrap_or_else(|| first_row_range(&range));
    let mut ops = vec![
        BatchOp::Format {
            range: header_range,
            style: StyleSpec {
                bold: Some(true),
                font_color: Some("FFFFFF".to_string()),
                fill: Some("4472C4".to_string()),
                ..StyleSpec::default()
            },
        },
        BatchOp::Format {
            range: range.clone(),
            style: StyleSpec {
                column_width: Some(14.0),
                ..StyleSpec::default()
            },
        },
    ];

    if let Some(number_format) = params.get("number_format") {
        ops.push(BatchOp::Format {
            range,
            style: StyleSpec {
                number_format: Some(number_format.clone()),
                ..StyleSpec::default()
            },
        });
    }

    Ok(ops)
}

fn required_param(params: &BTreeMap<String, String>, name: &str) -> Result<String, XliError> {
    params
        .get(name)
        .cloned()
        .ok_or_else(|| XliError::TemplateParamMissing {
            parameter: name.to_string(),
        })
}

fn first_row_range(range: &str) -> String {
    match xli_core::parse_range(range) {
        Ok(parsed) => {
            let sheet = parsed
                .sheet
                .as_ref()
                .map(|sheet| format!("{sheet}!"))
                .unwrap_or_default();
            format!(
                "{sheet}{}{}:{}{}",
                parsed.start.col, parsed.start.row, parsed.end.col, parsed.start.row
            )
        }
        Err(_) => range.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{expand_template, list_templates, preview_template};
    use std::collections::BTreeMap;
    use xli_core::BatchOp;

    #[test]
    fn lists_builtin_templates() {
        let templates = list_templates();
        assert!(templates
            .iter()
            .any(|template| template.name == "basic-table-format"));
    }

    #[test]
    fn expands_basic_table_format() {
        let mut params = BTreeMap::new();
        params.insert("range".to_string(), "Sheet1!A1:C10".to_string());
        params.insert("number_format".to_string(), "currency".to_string());

        let ops = expand_template("basic-table-format", &params).expect("expand");
        assert_eq!(ops.len(), 3);
        assert!(matches!(ops[0], BatchOp::Format { .. }));
    }

    #[test]
    fn preview_validates_required_params() {
        let params = BTreeMap::new();
        let error = preview_template("basic-table-format", &params).expect_err("missing param");
        assert!(matches!(
            error,
            xli_core::XliError::TemplateParamMissing { .. }
        ));
    }
}
