use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use xli_fs::{atomic_commit_with_options, AtomicCommitOptions};
use xli_ooxml::UMYA_FALLBACK_WARNING;

use crate::output;

#[derive(Debug, Args)]
pub struct WriteArgs {
    pub file: PathBuf,
    pub address: String,
    #[arg(long, group = "write_value")]
    pub value: Option<String>,
    #[arg(long, group = "write_value")]
    pub formula: Option<String>,
    #[arg(long)]
    pub sheet: Option<String>,
    #[arg(long)]
    pub expect_fingerprint: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
struct WriteOutput {
    written: u32,
    cells: Vec<String>,
    formulas_written: u32,
}

pub fn run(args: WriteArgs, human: bool) -> Result<bool> {
    let address = qualify_reference(&args.address, args.sheet.as_deref());
    let input = serde_json::json!({
        "file": args.file,
        "address": address,
        "value": args.value,
        "formula": args.formula,
        "dry_run": args.dry_run,
    });
    let parsed_value = args.value.as_deref().map(parse_cli_value);

    let result = atomic_commit_with_options(
        &args.file,
        args.expect_fingerprint.as_deref(),
        AtomicCommitOptions {
            dry_run: args.dry_run,
        },
        |src, dst| {
            xli_ooxml::apply_write(
                src,
                dst,
                &address,
                parsed_value.clone(),
                args.formula.clone(),
            )
        },
    );

    match result {
        Ok((commit, write_result)) => output::emit(
            &output::ok_envelope(
                "write",
                input,
                WriteOutput {
                    written: 1,
                    cells: vec![address],
                    formulas_written: u32::from(write_result.needs_recalc),
                },
                vec![UMYA_FALLBACK_WARNING.to_string()],
                write_result.needs_recalc,
                if args.dry_run {
                    xli_core::CommitMode::DryRun
                } else {
                    xli_core::CommitMode::Atomic
                },
                Some(commit.fingerprint_before),
                Some(commit.fingerprint_after),
                commit.stats,
            ),
            human,
        ),
        Err(error) => output::emit(
            &output::error_envelope::<WriteOutput>("write", Some(input), error),
            human,
        ),
    }
}

fn qualify_reference(reference: &str, sheet: Option<&str>) -> String {
    match sheet {
        Some(name) if !reference.contains('!') => format!("{name}!{reference}"),
        _ => reference.to_string(),
    }
}

fn parse_cli_value(value: &str) -> Value {
    serde_json::from_str::<Value>(value).unwrap_or_else(|_| Value::String(value.to_string()))
}
