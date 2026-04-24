use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use xli_new::{ColumnFormat, CsvCreateOptions};

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
    #[arg(long = "col")]
    pub columns: Vec<String>,
    #[arg(long = "hide")]
    pub hidden_columns: Vec<String>,
    #[arg(long = "rename")]
    pub renames: Vec<String>,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub total_row: bool,
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
        "columns": args.columns,
        "hidden_columns": args.hidden_columns,
        "renames": args.renames,
        "title": args.title,
        "total_row": args.total_row,
    });
    let result = if let Some(csv) = args.from_csv.as_deref() {
        let options = csv_options(&args);
        xli_new::create_from_csv_with_options(csv, &args.name, "Sheet1", &options).map(|_| 1)
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

fn csv_options(args: &CreateArgs) -> CsvCreateOptions {
    let mut selected = Vec::new();
    let mut formats = HashMap::new();
    for spec in &args.columns {
        let parts = spec.split(':').collect::<Vec<_>>();
        if let Some(name) = parts.first() {
            selected.push((*name).to_string());
            if parts.len() > 1 {
                formats.insert(
                    (*name).to_string(),
                    ColumnFormat {
                        number_format: Some(parts[1].to_string()),
                        alignment: parts.get(2).map(|value| (*value).to_string()),
                    },
                );
            }
        }
    }

    let renames = args
        .renames
        .iter()
        .filter_map(|spec| {
            spec.split_once(':')
                .map(|(from, to)| (from.to_string(), to.to_string()))
        })
        .collect();

    CsvCreateOptions {
        columns: if selected.is_empty() {
            None
        } else {
            Some(selected)
        },
        hidden_columns: args.hidden_columns.clone(),
        renames,
        formats,
        title: args.title.clone(),
        total_row: args.total_row,
    }
}
