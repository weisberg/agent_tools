use anyhow::Result;
use clap::{Args, Subcommand};
use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;
use xli_core::SheetAction;
use xli_fs::{atomic_commit_with_options, AtomicCommitOptions};
use xli_ooxml::UMYA_FALLBACK_WARNING;

use crate::output;

#[derive(Debug, Args)]
pub struct SheetArgs {
    pub file: PathBuf,
    #[arg(long)]
    pub expect_fingerprint: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[command(subcommand)]
    pub action: SheetCommand,
}

#[derive(Debug, Subcommand)]
pub enum SheetCommand {
    Add {
        name: String,
        #[arg(long)]
        after: Option<String>,
    },
    Remove {
        name: String,
    },
    Rename {
        from: String,
        #[arg(long)]
        to: String,
    },
    Copy {
        from: String,
        #[arg(long)]
        to: String,
    },
    Reorder {
        #[arg(long)]
        order: String,
    },
    Hide {
        name: String,
    },
    Unhide {
        name: String,
    },
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
struct SheetOutput {
    action: String,
}

pub fn run(args: SheetArgs, human: bool) -> Result<bool> {
    let action = match &args.action {
        SheetCommand::Add { name, after } => SheetAction::Add {
            name: name.clone(),
            after: after.clone(),
        },
        SheetCommand::Remove { name } => SheetAction::Delete { name: name.clone() },
        SheetCommand::Rename { from, to } => SheetAction::Rename {
            from: from.clone(),
            to: to.clone(),
        },
        SheetCommand::Copy { from, to } => SheetAction::Copy {
            from: from.clone(),
            to: to.clone(),
        },
        SheetCommand::Reorder { order } => SheetAction::Reorder {
            sheets: order
                .split(',')
                .map(|item| item.trim().to_string())
                .collect(),
        },
        SheetCommand::Hide { name } => SheetAction::Hide { name: name.clone() },
        SheetCommand::Unhide { name } => SheetAction::Unhide { name: name.clone() },
    };
    let input = serde_json::json!({ "file": args.file, "action": action });

    let result = atomic_commit_with_options(
        &args.file,
        args.expect_fingerprint.as_deref(),
        AtomicCommitOptions {
            dry_run: args.dry_run,
        },
        |src, dst| {
            xli_ooxml::apply_sheet_action(src, dst, &action)?;
            Ok::<_, xli_core::XliError>(())
        },
    );

    match result {
        Ok((commit, ())) => output::emit(
            &output::ok_envelope(
                "sheet",
                input,
                SheetOutput {
                    action: format!("{:?}", args.action),
                },
                vec![UMYA_FALLBACK_WARNING.to_string()],
                false,
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
            &output::error_envelope::<SheetOutput>("sheet", Some(input), error),
            human,
        ),
    }
}
