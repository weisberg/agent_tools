use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;
use xli_core::StyleSpec;
use xli_fs::{atomic_commit_with_options, AtomicCommitOptions};
use xli_ooxml::UMYA_FALLBACK_WARNING;

use crate::output;

#[derive(Debug, Args)]
pub struct FormatArgs {
    pub file: PathBuf,
    pub range: String,
    #[arg(long)]
    pub sheet: Option<String>,
    #[arg(long)]
    pub bold: bool,
    #[arg(long)]
    pub italic: bool,
    #[arg(long)]
    pub font_color: Option<String>,
    #[arg(long)]
    pub fill: Option<String>,
    #[arg(long)]
    pub number_format: Option<String>,
    #[arg(long)]
    pub column_width: Option<f64>,
    #[arg(long)]
    pub expect_fingerprint: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
struct FormatOutput {
    cells_formatted: u32,
}

pub fn run(args: FormatArgs, human: bool) -> Result<bool> {
    let range = qualify_reference(&args.range, args.sheet.as_deref());
    let style = StyleSpec {
        bold: args.bold.then_some(true),
        italic: args.italic.then_some(true),
        font_color: args.font_color.clone(),
        fill: args.fill.clone(),
        number_format: args.number_format.clone(),
        column_width: args.column_width,
        horizontal_align: None,
        vertical_align: None,
    };
    let input = serde_json::json!({
        "file": args.file,
        "range": range,
        "style": style,
        "dry_run": args.dry_run,
    });

    let result = atomic_commit_with_options(
        &args.file,
        args.expect_fingerprint.as_deref(),
        AtomicCommitOptions {
            dry_run: args.dry_run,
        },
        |src, dst| {
            xli_ooxml::apply_format(src, dst, &range, &style)?;
            Ok::<_, xli_core::XliError>(())
        },
    );

    match result {
        Ok((commit, ())) => output::emit(
            &output::ok_envelope(
                "format",
                input,
                FormatOutput {
                    cells_formatted: formatted_cells(&range).unwrap_or(0),
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
            &output::error_envelope::<FormatOutput>("format", Some(input), error),
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

fn formatted_cells(range: &str) -> Result<u32, xli_core::XliError> {
    let range_ref = xli_core::parse_range(range).map_err(xli_core::XliError::from)?;
    // Use checked_sub to avoid u32 underflow on inverted ranges. (Issue #20)
    let width = range_ref
        .end
        .col_idx
        .checked_sub(range_ref.start.col_idx)
        .ok_or_else(|| xli_core::XliError::InvalidCellAddress {
            address: range.to_string(),
        })?
        + 1;
    let height = range_ref
        .end
        .row
        .checked_sub(range_ref.start.row)
        .ok_or_else(|| xli_core::XliError::InvalidCellAddress {
            address: range.to_string(),
        })?
        + 1;
    Ok(width * height)
}
