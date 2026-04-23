use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;

use crate::output;

#[derive(Debug, Args)]
pub struct CreateArgs {
    pub name: PathBuf,
    #[arg(long)]
    pub sheets: Option<String>,
    #[arg(long)]
    pub from_csv: Option<PathBuf>,
    #[arg(long)]
    pub from_markdown: Option<PathBuf>,
    #[arg(long)]
    pub from_json: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
struct CreateOutput {
    file: String,
    sheets_created: usize,
}

pub fn run(args: CreateArgs, human: bool) -> Result<bool> {
    let input = serde_json::json!({
        "name": args.name,
        "sheets": args.sheets,
        "from_csv": args.from_csv,
        "from_markdown": args.from_markdown,
        "from_json": args.from_json,
    });
    let result = if let Some(csv) = args.from_csv.as_deref() {
        xli_new::create_from_csv(csv, &args.name, "Sheet1").map(|_| 1)
    } else if let Some(md) = args.from_markdown.as_deref() {
        xli_new::create_from_markdown(md, &args.name, "Sheet1").map(|_| 1)
    } else if let Some(json) = args.from_json.as_deref() {
        xli_new::create_from_json(json, &args.name)
    } else {
        let sheets = args
            .sheets
            .as_deref()
            .map(|value| {
                value
                    .split(',')
                    .map(|item| item.trim().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let count = if sheets.is_empty() { 1 } else { sheets.len() };
        xli_new::create_blank(&args.name, &sheets).map(|_| count)
    };

    match result {
        Ok(sheets_created) => output::emit(
            &output::ok_envelope(
                "create",
                input,
                CreateOutput {
                    file: args.name.display().to_string(),
                    sheets_created,
                },
                Vec::new(),
                false,
                xli_core::CommitMode::None,
                None,
                None,
                xli_core::CommitStats::default(),
            ),
            human,
        ),
        Err(error) => output::emit(
            &output::error_envelope::<CreateOutput>("create", Some(input), error),
            human,
        ),
    }
}
