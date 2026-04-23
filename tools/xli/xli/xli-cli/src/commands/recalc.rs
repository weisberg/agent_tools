use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

use crate::output;

#[derive(Debug, Args)]
pub struct RecalcArgs {
    pub file: PathBuf,
    #[arg(long, default_value = "30")]
    pub timeout: u64,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct RecalcOutput {
    pub result: xli_calc::RecalcResult,
}

pub fn run(args: RecalcArgs, human: bool) -> Result<bool> {
    let input = serde_json::json!({
        "file": args.file,
        "timeout": args.timeout,
    });
    let result = xli_fs::atomic_commit(&args.file, None, |src, dst| {
        fs::copy(src, dst).map_err(|error| xli_core::XliError::WriteConflict {
            target: dst.display().to_string(),
            details: Some(error.to_string()),
        })?;
        xli_calc::recalc(dst, args.timeout)
    });

    match result {
        Ok((commit, recalc_result)) => output::emit(
            &output::ok_envelope(
                "recalc",
                input,
                RecalcOutput {
                    result: recalc_result,
                },
                Vec::new(),
                false,
                xli_core::CommitMode::Atomic,
                Some(commit.fingerprint_before),
                Some(commit.fingerprint_after),
                commit.stats,
            ),
            human,
        ),
        Err(error) => output::emit(
            &output::error_envelope::<RecalcOutput>("recalc", Some(input), error),
            human,
        ),
    }
}
