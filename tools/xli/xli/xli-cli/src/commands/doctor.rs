use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;

use crate::commands::{lint, validate};
use crate::output;

#[derive(Debug, Args)]
pub struct DoctorArgs {
    pub file: PathBuf,
    #[arg(long)]
    pub skip_recalc: bool,
    #[arg(long, default_value = "30")]
    pub timeout: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct DoctorOutput {
    pub lint: lint::LintOutput,
    pub recalc: Option<xli_calc::RecalcResult>,
    pub validate: validate::ValidateOutput,
}

pub fn run(args: DoctorArgs, human: bool) -> Result<bool> {
    let input = serde_json::json!({
        "file": args.file,
        "skip_recalc": args.skip_recalc,
        "timeout": args.timeout,
    });

    let envelope = match run_doctor(&args) {
        Ok(output_data) => {
            let mut envelope = output::ok_envelope(
                "doctor",
                input,
                output_data,
                Vec::new(),
                false,
                xli_core::CommitMode::None,
                None,
                None,
                xli_core::CommitStats::default(),
            );

            if !envelope
                .output
                .as_ref()
                .is_some_and(|output| output.lint.issues.is_empty() && output.validate.clean)
            {
                envelope.status = xli_core::Status::IssuesFound;
            }

            envelope
        }
        Err(error) => output::error_envelope("doctor", Some(input), error),
    };

    output::emit(&envelope, human)
}

fn run_doctor(args: &DoctorArgs) -> Result<DoctorOutput, xli_core::XliError> {
    let lint = lint::lint_workbook(&args.file, None, None)?;
    let recalc = if args.skip_recalc {
        None
    } else {
        Some(xli_calc::recalc(&args.file, args.timeout)?)
    };
    let validate = validate::scan_errors(&args.file)?;

    Ok(DoctorOutput {
        lint,
        recalc,
        validate,
    })
}
