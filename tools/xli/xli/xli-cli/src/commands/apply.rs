use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;
use xli_fs::{atomic_commit_with_options, AtomicCommitOptions};
use xli_ooxml::{BatchSummary, UMYA_FALLBACK_WARNING};

use crate::commands::template::parse_params;
use crate::output;

#[derive(Debug, Args)]
pub struct ApplyArgs {
    pub file: PathBuf,
    pub template: String,
    #[arg(long = "param")]
    pub params: Vec<String>,
    #[arg(long)]
    pub expect_fingerprint: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
struct ApplyOutput {
    template: String,
    ops_executed: usize,
    stats: BatchSummary,
}

pub fn run(args: ApplyArgs, human: bool) -> Result<bool> {
    let params = parse_params(&args.params);
    let ops = match xli_kb::expand_template(&args.template, &params) {
        Ok(ops) => ops,
        Err(error) => {
            return output::emit(
                &output::error_envelope::<ApplyOutput>(
                    "apply",
                    Some(serde_json::json!({
                        "file": args.file,
                        "template": args.template,
                        "params": params,
                    })),
                    error,
                ),
                human,
            );
        }
    };
    let input = serde_json::json!({
        "file": args.file,
        "template": args.template,
        "params": params,
        "dry_run": args.dry_run,
    });

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
                "apply",
                input,
                ApplyOutput {
                    template: args.template,
                    ops_executed: summary.ops_executed,
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
            &output::error_envelope::<ApplyOutput>("apply", Some(input), error),
            human,
        ),
    }
}
