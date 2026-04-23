use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::path::PathBuf;
use xli_core::{CommitMode, CommitStats, Status};

use crate::output;

#[derive(Debug, Args)]
pub struct LintArgs {
    pub file: PathBuf,
    #[arg(long)]
    pub rules: Option<String>,
    #[arg(long)]
    pub severity: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct LintRepair {
    pub description: String,
    pub op: xli_core::BatchOp,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct LintIssue {
    pub rule: String,
    pub severity: String,
    pub cell: String,
    pub message: String,
    pub suggested_repair: Option<LintRepair>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct LintOutput {
    pub issues: Vec<LintIssue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScannedCell {
    row: u32,
    col: String,
    address: String,
    formula: Option<String>,
}

pub fn run(args: LintArgs, human: bool) -> Result<bool> {
    let input = serde_json::json!({
        "file": args.file,
        "rules": args.rules,
        "severity": args.severity,
    });

    let envelope = match lint_workbook(&args.file, args.rules.as_deref(), args.severity.as_deref())
    {
        Ok(output_data) => {
            let mut envelope = output::ok_envelope(
                "lint",
                input,
                output_data,
                Vec::new(),
                false,
                CommitMode::None,
                None,
                None,
                CommitStats::default(),
            );

            if !envelope
                .output
                .as_ref()
                .is_some_and(|output| output.issues.is_empty())
            {
                envelope.status = Status::IssuesFound;
            }
            envelope
        }
        Err(error) => output::error_envelope("lint", Some(input), error),
    };

    output::emit(&envelope, human)
}

pub fn lint_workbook(
    path: &Path,
    rules_filter: Option<&str>,
    severity_filter: Option<&str>,
) -> Result<LintOutput, xli_core::XliError> {
    let workbook = xli_read::inspect(path)?;
    let include_rules = parse_list_filter(rules_filter);
    let include_severity = severity_filter
        .as_ref()
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase());

    let mut issues = Vec::new();

    for sheet in workbook.sheets {
        let Some(dimensions) = sheet.dimensions.as_deref() else {
            continue;
        };

        let range = xli_core::parse_range(&format!("{}!{dimensions}", sheet.name))
            .map_err(xli_core::XliError::from)?;
        let mut scanned = Vec::new();

        for row in range.start.row..=range.end.row {
            for col_idx in range.start.col_idx..=range.end.col_idx {
                let address = format!("{}!{}{}", sheet.name, xli_core::col_to_letter(col_idx), row);
                let cell = xli_read::read_cell(path, &address)?;

                scanned.push(ScannedCell {
                    row,
                    col: xli_core::col_to_letter(col_idx),
                    address,
                    formula: cell.formula,
                });
            }
        }

        issues.extend(lint_formulas(&scanned));
        issues.extend(lint_duplicate_headers(&sheet.name, dimensions, path)?);
    }

    let filtered = issues
        .into_iter()
        .filter(|issue| {
            include_rules
                .as_ref()
                .is_none_or(|rules| rules.contains(issue.rule.as_str()))
        })
        .filter(|issue| {
            include_severity
                .as_ref()
                .is_none_or(|severity| severity == issue.severity.as_str())
        })
        .collect::<Vec<_>>();

    Ok(LintOutput { issues: filtered })
}

fn parse_list_filter(value: Option<&str>) -> Option<BTreeSet<String>> {
    value.map(|value| {
        value
            .split(',')
            .map(|item| item.trim().to_lowercase())
            .filter(|item| !item.is_empty())
            .collect()
    })
}

fn lint_formulas(cells: &[ScannedCell]) -> Vec<LintIssue> {
    let mut issues = Vec::new();

    for cell in cells {
        let Some(formula) = cell.formula.as_deref() else {
            continue;
        };

        if formula.contains("_xlfn.") {
            issues.push(LintIssue {
                rule: "formula-prefix-xlfn".to_string(),
                severity: "warning".to_string(),
                cell: cell.address.clone(),
                message: "formula uses legacy _xlfn. prefix".to_string(),
                suggested_repair: Some(LintRepair {
                    description: "Strip the _xlfn. prefix".to_string(),
                    op: xli_core::BatchOp::Write {
                        address: cell.address.clone(),
                        value: None,
                        formula: Some(formula.replace("_xlfn.", "")),
                    },
                }),
            });
        }

        if formula.contains("_xlpm.") {
            issues.push(LintIssue {
                rule: "formula-prefix-xlpm".to_string(),
                severity: "warning".to_string(),
                cell: cell.address.clone(),
                message: "formula uses legacy _xlpm. prefix".to_string(),
                suggested_repair: Some(LintRepair {
                    description: "Strip the _xlpm. prefix".to_string(),
                    op: xli_core::BatchOp::Write {
                        address: cell.address.clone(),
                        value: None,
                        formula: Some(formula.replace("_xlpm.", "")),
                    },
                }),
            });
        }

        let local_addr = format!("{}{}", cell.col, cell.row);
        if formula
            .to_ascii_uppercase()
            .contains(&local_addr.to_ascii_uppercase())
        {
            issues.push(LintIssue {
                rule: "circular-ref-suspect".to_string(),
                severity: "error".to_string(),
                cell: cell.address.clone(),
                message: "formula references its own cell address".to_string(),
                suggested_repair: None,
            });
        }

        if formula.len() > 500 {
            issues.push(LintIssue {
                rule: "very-long-formula".to_string(),
                severity: "info".to_string(),
                cell: cell.address.clone(),
                message: "formula is very long".to_string(),
                suggested_repair: None,
            });
        }
    }

    issues
}

fn lint_duplicate_headers(
    sheet_name: &str,
    dimensions: &str,
    path: &Path,
) -> Result<Vec<LintIssue>, xli_core::XliError> {
    let range =
        xli_core::parse_range(&format!("{}!{dimensions}", sheet_name)).map_err(|error| {
            xli_core::XliError::TemplateParamInvalid {
                parameter: "dimensions".to_string(),
                details: error.to_string(),
            }
        })?;
    let header_range = format!(
        "{sheet_name}!{}{}:{}{}",
        range.start.col, range.start.row, range.end.col, range.start.row
    );
    let headers = xli_read::read_range(path, &header_range, None, None, false)?;
    let row = headers.rows.first().cloned().unwrap_or_default();
    let mut seen: HashMap<String, Vec<String>> = HashMap::new();
    let mut col = range.start.col_idx;
    for value in row.values() {
        if let Some(text) = value
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            let address = format!(
                "{}!{}{}",
                sheet_name,
                xli_core::col_to_letter(col),
                range.start.row
            );
            seen.entry(text.to_string()).or_default().push(address);
        }
        col += 1;
    }

    let mut issues = Vec::new();
    for (value, cells) in seen {
        if cells.len() > 1 {
            for cell in cells {
                issues.push(LintIssue {
                    rule: "duplicate-header".to_string(),
                    severity: "info".to_string(),
                    cell,
                    message: format!("header value `{value}` is duplicated"),
                    suggested_repair: None,
                });
            }
        }
    }

    Ok(issues)
}
