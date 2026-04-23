use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use xli_core::{CommitMode, CommitStats, Status};

use crate::output;

#[derive(Debug, Args)]
pub struct ValidateArgs {
    pub file: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ValidationError {
    pub cell: String,
    pub error: String,
    pub formula: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ValidateOutput {
    pub errors: Vec<ValidationError>,
    pub clean: bool,
    pub scanned_cells: usize,
}

pub fn run(args: ValidateArgs, human: bool) -> Result<bool> {
    let input = serde_json::json!({ "file": args.file });

    let envelope = match scan_errors(&args.file) {
        Ok(validate_output) => {
            let mut envelope = output::ok_envelope(
                "validate",
                input,
                validate_output,
                Vec::new(),
                false,
                CommitMode::None,
                None,
                None,
                CommitStats::default(),
            );

            if envelope.output.as_ref().is_some_and(|output| !output.clean) {
                envelope.status = Status::IssuesFound;
            }
            envelope
        }
        Err(error) => output::error_envelope("validate", Some(input), error),
    };

    output::emit(&envelope, human)
}

pub fn scan_errors(path: &Path) -> Result<ValidateOutput, xli_core::XliError> {
    let workbook = xli_read::inspect(path)?;
    let mut errors = Vec::new();
    let mut scanned_cells = 0usize;

    for sheet in workbook.sheets {
        let Some(dimensions) = sheet.dimensions.as_deref() else {
            continue;
        };

        let range = xli_core::parse_range(&format!("{}!{dimensions}", sheet.name))
            .map_err(xli_core::XliError::from)?;

        for row in range.start.row..=range.end.row {
            for col_idx in range.start.col_idx..=range.end.col_idx {
                let address = format!("{}!{}{}", sheet.name, xli_core::col_to_letter(col_idx), row);
                let cell = xli_read::read_cell(path, &address)?;
                scanned_cells += 1;

                if let Some(value) = cell.value.as_str().filter(|error| is_error_value(error)) {
                    errors.push(ValidationError {
                        cell: address,
                        error: value.to_string(),
                        formula: cell.formula,
                    });
                }
            }
        }
    }

    Ok(ValidateOutput {
        clean: errors.is_empty(),
        errors,
        scanned_cells,
    })
}

fn is_error_value(value: &str) -> bool {
    matches!(
        value,
        "#REF!" | "#DIV/0!" | "#N/A" | "#NAME?" | "#VALUE!" | "#NULL!" | "#NUM!"
    )
}
