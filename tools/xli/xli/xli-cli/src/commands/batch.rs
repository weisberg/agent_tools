use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use xli_core::BatchOp;
use xli_fs::{atomic_commit_with_options, AtomicCommitOptions};
use xli_ooxml::{BatchSummary, UMYA_FALLBACK_WARNING};

use crate::output;

#[derive(Debug, Args)]
pub struct BatchArgs {
    pub file: PathBuf,
    #[arg(long)]
    pub stdin: bool,
    #[arg(long)]
    pub file_input: Option<PathBuf>,
    #[arg(long)]
    pub expect_fingerprint: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
struct BatchOutput {
    ops_executed: usize,
    ops_failed: usize,
    stats: BatchSummary,
}

pub fn run(args: BatchArgs, human: bool) -> Result<bool> {
    let contents = if args.stdin {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        buffer
    } else if let Some(path) = args.file_input.as_deref() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };

    let ops = match parse_ops(&contents) {
        Ok(ops) => ops,
        Err(error) => {
            return output::emit(
                &output::error_envelope::<BatchOutput>(
                    "batch",
                    Some(serde_json::json!({ "file": args.file })),
                    error,
                ),
                human,
            );
        }
    };
    let input = serde_json::json!({ "file": args.file, "ops": ops, "dry_run": args.dry_run });
    let result = atomic_commit_with_options(
        &args.file,
        args.expect_fingerprint.as_deref(),
        AtomicCommitOptions {
            dry_run: args.dry_run,
        },
        |src, dst| xli_ooxml::apply_batch(src, dst, &ops),
    );

    match result {
        Ok((commit, (summary, needs_recalc))) => output::emit(
            &output::ok_envelope(
                "batch",
                input,
                BatchOutput {
                    ops_executed: summary.ops_executed,
                    ops_failed: 0,
                    stats: summary,
                },
                vec![UMYA_FALLBACK_WARNING.to_string()],
                needs_recalc,
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
            &output::error_envelope::<BatchOutput>("batch", Some(input), error),
            human,
        ),
    }
}

fn parse_ops(contents: &str) -> Result<Vec<BatchOp>, xli_core::XliError> {
    let mut ops = Vec::new();
    for line in contents.lines().filter(|line| !line.trim().is_empty()) {
        let op = serde_json::from_str::<BatchOp>(line).map_err(|error| {
            xli_core::XliError::CliParseError {
                message: error.to_string(),
            }
        })?;
        ops.push(op);
    }
    Ok(ops)
}
