use anyhow::Result;
use clap::Args;
use serde_json::Value;
use std::io::{self, Write};
use std::path::PathBuf;
use xli_read::{CellData, RangeData};

use crate::output;

#[derive(Debug, Args)]
pub struct ReadArgs {
    pub file: PathBuf,
    pub address: Option<String>,
    #[arg(long)]
    pub sheet: Option<String>,
    #[arg(long)]
    pub range: Option<String>,
    #[arg(long)]
    pub table: Option<String>,
    #[arg(long, default_value = "1000")]
    pub limit: usize,
    #[arg(long, default_value = "0")]
    pub offset: usize,
    #[arg(long)]
    pub formulas: bool,
    #[arg(long)]
    pub headers: bool,
    /// Output format: json (default) or markdown
    #[arg(long, default_value = "json")]
    pub format: String,
}

pub fn run(args: ReadArgs, human: bool) -> Result<bool> {
    // json-full whole-workbook export: only when no specific address/range/table
    if args.format == "json-full"
        && args.address.is_none()
        && args.range.is_none()
        && args.table.is_none()
    {
        let result = xli_read::read_all_sheets(&args.file)?;
        let stdout = io::stdout();
        let mut out = stdout.lock();
        serde_json::to_writer_pretty(&mut out, &result)?;
        writeln!(out)?;
        return Ok(false);
    }

    let markdown = args.format == "markdown";
    let use_headers = args.headers || markdown;

    let input = serde_json::json!({
        "file": args.file,
        "address": args.address,
        "sheet": args.sheet,
        "range": args.range,
        "table": args.table,
        "limit": args.limit,
        "offset": args.offset,
        "headers": use_headers,
        "formulas": args.formulas,
        "format": args.format,
    });

    if let Some(table_name) = args.table.as_deref() {
        return match xli_read::read_table(
            &args.file,
            table_name,
            Some(args.limit),
            Some(args.offset),
        ) {
            Ok(output_data) => {
                if markdown {
                    return emit_markdown(&output_data);
                }
                output::emit(
                    &output::ok_envelope(
                        "read",
                        input,
                        output_data,
                        Vec::new(),
                        false,
                        xli_core::CommitMode::None,
                        None,
                        None,
                        xli_core::CommitStats::default(),
                    ),
                    human,
                )
            }
            Err(error) => output::emit(
                &output::error_envelope::<RangeData>("read", Some(input), error),
                human,
            ),
        };
    }

    if let Some(range) = args.range.as_deref().or(args.address.as_deref()) {
        let reference = qualify_reference(range, args.sheet.as_deref());
        if reference.contains(':') {
            return match xli_read::read_range(
                &args.file,
                &reference,
                Some(args.limit),
                Some(args.offset),
                use_headers,
            ) {
                Ok(output_data) => {
                    if markdown {
                        return emit_markdown(&output_data);
                    }
                    output::emit(
                        &output::ok_envelope(
                            "read",
                            input,
                            output_data,
                            Vec::new(),
                            false,
                            xli_core::CommitMode::None,
                            None,
                            None,
                            xli_core::CommitStats::default(),
                        ),
                        human,
                    )
                }
                Err(error) => output::emit(
                    &output::error_envelope::<RangeData>("read", Some(input), error),
                    human,
                ),
            };
        }

        return match xli_read::read_cell(&args.file, &reference) {
            Ok(output_data) => {
                let output_data = if args.formulas {
                    formula_view(output_data)
                } else {
                    output_data
                };
                output::emit(
                    &output::ok_envelope(
                        "read",
                        input,
                        output_data,
                        Vec::new(),
                        false,
                        xli_core::CommitMode::None,
                        None,
                        None,
                        xli_core::CommitStats::default(),
                    ),
                    human,
                )
            }
            Err(error) => output::emit(
                &output::error_envelope::<CellData>("read", Some(input), error),
                human,
            ),
        };
    }

    output::emit(
        &output::error_envelope::<serde_json::Value>(
            "read",
            Some(input),
            xli_core::XliError::CliParseError {
                message: "read requires an address, --range, or --table".to_string(),
            },
        ),
        human,
    )
}

fn emit_markdown(data: &RangeData) -> Result<bool> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Determine column order from headers or first row keys
    let columns: Vec<String> = if let Some(headers) = &data.headers {
        headers.clone()
    } else if let Some(first) = data.rows.first() {
        let mut keys: Vec<String> = first.keys().cloned().collect();
        keys.sort();
        keys
    } else {
        return Ok(false);
    };

    // Compute column widths (min 3 for the separator dashes)
    let widths: Vec<usize> = columns
        .iter()
        .map(|col| {
            let header_w = col.len();
            let max_cell = data
                .rows
                .iter()
                .map(|row| format_cell(row.get(col)).len())
                .max()
                .unwrap_or(0);
            header_w.max(max_cell).max(3)
        })
        .collect();

    // Header row
    write!(out, "|")?;
    for (col, w) in columns.iter().zip(&widths) {
        write!(out, " {:<w$} |", col, w = w)?;
    }
    writeln!(out)?;

    // Separator row
    write!(out, "|")?;
    for w in &widths {
        write!(out, " {:-<w$} |", "", w = w)?;
    }
    writeln!(out)?;

    // Data rows
    for row in &data.rows {
        write!(out, "|")?;
        for (col, w) in columns.iter().zip(&widths) {
            let cell = format_cell(row.get(col));
            write!(out, " {:<w$} |", cell, w = w)?;
        }
        writeln!(out)?;
    }

    Ok(false)
}

fn format_cell(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(other) => other.to_string(),
    }
}

fn qualify_reference(reference: &str, sheet: Option<&str>) -> String {
    match sheet {
        Some(name) if !reference.contains('!') => format!("{name}!{reference}"),
        _ => reference.to_string(),
    }
}

fn formula_view(mut cell: CellData) -> CellData {
    if let Some(formula) = cell.formula.clone() {
        cell.value = serde_json::Value::String(formula);
        cell.value_type = xli_read::CellValueType::Formula;
    }
    cell
}
