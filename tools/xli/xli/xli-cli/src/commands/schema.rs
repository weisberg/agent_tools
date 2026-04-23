use anyhow::Result;
use clap::Args;
use std::io::{self, Write};

use crate::output;

#[derive(Debug, Args)]
pub struct SchemaArgs {
    #[arg(long)]
    pub command: Option<String>,
    #[arg(long)]
    pub result: Option<String>,
    #[arg(long)]
    pub openapi: bool,
}

pub fn run(args: SchemaArgs, human: bool) -> Result<bool> {
    let schema = match (&args.command, &args.result, args.openapi) {
        (None, None, true) => Ok(xli_schema::emit_openapi()),
        (Some(_), Some(_), _) => Err(xli_core::XliError::CliParseError {
            message: "--command and --result are mutually exclusive".to_string(),
        }),
        (Some(command), None, false) => xli_schema::emit_command_schema(command),
        (None, Some(result), false) => emit_result_schema(result),
        (None, None, false) => Ok(xli_schema::emit_full_schema()),
        (Some(_), None, true) => Err(xli_core::XliError::CliParseError {
            message: "--openapi cannot be combined with --command".to_string(),
        }),
        (None, Some(_), true) => Err(xli_core::XliError::CliParseError {
            message: "--openapi cannot be combined with --result".to_string(),
        }),
    };

    match schema {
        Ok(value) => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            if human {
                serde_json::to_writer_pretty(&mut out, &value)?;
            } else {
                serde_json::to_writer(&mut out, &value)?;
            }
            writeln!(out)?;
            Ok(false)
        }
        Err(error) => output::emit(
            &output::error_envelope::<serde_json::Value>("schema", None, error),
            human,
        ),
    }
}

fn emit_result_schema(name: &str) -> Result<serde_json::Value, xli_core::XliError> {
    let full = xli_schema::emit_full_schema();
    full.get("results")
        .and_then(|results| results.get(name))
        .cloned()
        .ok_or_else(|| xli_core::XliError::TemplateParamInvalid {
            parameter: "result".to_string(),
            details: format!("Unknown result schema: {name}"),
        })
}
