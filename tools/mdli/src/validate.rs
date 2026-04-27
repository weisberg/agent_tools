use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::*;

pub(crate) fn run_validate(args: ValidateArgs) -> Result<Outcome, MdliError> {
    let doc = MarkdownDocument::read(&args.file)?;
    let schema = load_validation_schema(&args.schema)?;
    schema.validate_self()?;
    let index = index_document(&doc);
    let mut findings: Vec<Value> = Vec::new();

    for required in &schema.required_sections {
        match index
            .sections
            .iter()
            .find(|s| s.id.as_deref() == Some(required.id.as_str()))
        {
            None => findings.push(json!({
                "rule": "required-section",
                "severity": "error",
                "code": "E_VALIDATION_MISSING_SECTION",
                "message": format!("required section {} is missing", required.id),
                "id": required.id,
            })),
            Some(section) => {
                if let Some(level) = required.level {
                    if section.level != level {
                        findings.push(json!({
                            "rule": "required-section-level",
                            "severity": "error",
                            "code": "E_VALIDATION_SECTION_LEVEL",
                            "message": format!(
                                "section {} is level {} but schema requires {}",
                                required.id, section.level, level
                            ),
                            "id": required.id,
                            "actual": section.level,
                            "expected": level,
                        }));
                    }
                }
            }
        }
    }

    for required in &schema.required_tables {
        match index
            .tables
            .iter()
            .find(|t| t.name.as_deref() == Some(required.name.as_str()))
        {
            None => findings.push(json!({
                "rule": "required-table",
                "severity": "error",
                "code": "E_VALIDATION_MISSING_TABLE",
                "message": format!("required table {} is missing", required.name),
                "name": required.name,
            })),
            Some(table) => {
                if let Some(expected) = &required.columns {
                    if &table.columns != expected {
                        findings.push(json!({
                            "rule": "required-table-columns",
                            "severity": "error",
                            "code": "E_VALIDATION_TABLE_COLUMNS",
                            "message": format!(
                                "table {} columns do not match schema",
                                required.name
                            ),
                            "name": required.name,
                            "actual": table.columns,
                            "expected": expected,
                        }));
                    }
                }
                if let Some(expected_key) = &required.key {
                    if table.key.as_ref() != Some(expected_key) {
                        findings.push(json!({
                            "rule": "required-table-key",
                            "severity": "error",
                            "code": "E_VALIDATION_TABLE_KEY",
                            "message": format!(
                                "table {} key does not match schema",
                                required.name
                            ),
                            "name": required.name,
                            "actual": table.key,
                            "expected": expected_key,
                        }));
                    }
                }
            }
        }
    }

    for required in &schema.managed_blocks {
        match index.blocks.iter().find(|b| b.id == required.id) {
            None => findings.push(json!({
                "rule": "required-managed-block",
                "severity": "error",
                "code": "E_VALIDATION_MISSING_BLOCK",
                "message": format!("required managed block {} is missing", required.id),
                "id": required.id,
            })),
            Some(block) => {
                if let Some(expected_locked) = required.locked {
                    if block.locked != expected_locked {
                        findings.push(json!({
                            "rule": "required-managed-block-lock",
                            "severity": "error",
                            "code": "E_VALIDATION_BLOCK_LOCK",
                            "message": format!(
                                "block {} locked={} but schema requires {}",
                                required.id, block.locked, expected_locked
                            ),
                            "id": required.id,
                            "actual": block.locked,
                            "expected": expected_locked,
                        }));
                    }
                }
            }
        }
    }

    let ok = findings.is_empty();
    Ok(Outcome::Json(json!({
        "schema": schema.schema,
        "ok": ok,
        "findings": findings,
    })))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ValidationSchema {
    #[serde(default)]
    pub(crate) schema: String,
    #[serde(default)]
    pub(crate) required_sections: Vec<RequiredSection>,
    #[serde(default)]
    pub(crate) required_tables: Vec<RequiredTable>,
    #[serde(default)]
    pub(crate) managed_blocks: Vec<RequiredManagedBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RequiredSection {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) level: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RequiredTable {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) columns: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RequiredManagedBlock {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) locked: Option<bool>,
}

impl ValidationSchema {
    pub(crate) fn validate_self(&self) -> Result<(), MdliError> {
        if self.schema != "mdli/validation/v1" {
            return Err(MdliError::user(
                "E_VALIDATION_SCHEMA_INVALID",
                format!(
                    "expected schema mdli/validation/v1, got {}",
                    if self.schema.is_empty() {
                        "<missing>"
                    } else {
                        self.schema.as_str()
                    }
                ),
            ));
        }
        Ok(())
    }
}

pub(crate) fn load_validation_schema(path: &Path) -> Result<ValidationSchema, MdliError> {
    let text = fs::read_to_string(path).map_err(|e| {
        MdliError::io(
            "E_READ_FAILED",
            format!("failed to read schema {}", path.display()),
            e,
        )
    })?;
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') {
        serde_json::from_str(&text).map_err(|e| {
            MdliError::user(
                "E_VALIDATION_SCHEMA_INVALID",
                format!("invalid JSON schema: {e}"),
            )
        })
    } else {
        serde_yaml::from_str(&text).map_err(|e| {
            MdliError::user(
                "E_VALIDATION_SCHEMA_INVALID",
                format!("invalid YAML schema: {e}"),
            )
        })
    }
}
