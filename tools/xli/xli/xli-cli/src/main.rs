#![forbid(unsafe_code)]

mod commands;
mod output;

use clap::{Parser, Subcommand};
use commands::{
    apply::ApplyArgs, batch::BatchArgs, create::CreateArgs, doctor::DoctorArgs, format::FormatArgs,
    inspect::InspectArgs, lint::LintArgs, read::ReadArgs, recalc::RecalcArgs, schema::SchemaArgs,
    sheet::SheetArgs, template::TemplateArgs, validate::ValidateArgs, write::WriteArgs,
};
use schemars::JsonSchema;
use serde::Serialize;
use xli_core::{CommitMode, CommitStats, ResponseEnvelope, Status, XliError};

#[derive(Debug, Parser)]
#[command(
    name = "xli",
    version,
    about = "Excel CLI for structured workbook operations"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, global = true)]
    human: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Inspect(InspectArgs),
    Read(ReadArgs),
    Write(WriteArgs),
    Format(FormatArgs),
    Sheet(SheetArgs),
    Batch(BatchArgs),
    Apply(ApplyArgs),
    Create(CreateArgs),
    Lint(LintArgs),
    Recalc(RecalcArgs),
    Validate(ValidateArgs),
    Doctor(DoctorArgs),
    Template(TemplateArgs),
    Schema(SchemaArgs),
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            if !std::env::args().any(|arg| arg == "--human") {
                let envelope: ResponseEnvelope<serde_json::Value> =
                    make_error_envelope(XliError::CliParseError {
                        message: error.to_string(),
                    });
                let _ = output::emit(&envelope, false);
                std::process::exit(2);
            }
            error.exit();
        }
    };

    let exit_code = match run(cli) {
        Ok(is_error) => i32::from(is_error),
        Err(error) => {
            eprintln!("{error}");
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(cli: Cli) -> anyhow::Result<bool> {
    match cli.command {
        Commands::Inspect(args) => commands::inspect::run(args, cli.human),
        Commands::Read(args) => commands::read::run(args, cli.human),
        Commands::Write(args) => commands::write::run(args, cli.human),
        Commands::Format(args) => commands::format::run(args, cli.human),
        Commands::Sheet(args) => commands::sheet::run(args, cli.human),
        Commands::Batch(args) => commands::batch::run(args, cli.human),
        Commands::Apply(args) => commands::apply::run(args, cli.human),
        Commands::Create(args) => commands::create::run(args, cli.human),
        Commands::Lint(args) => commands::lint::run(args, cli.human),
        Commands::Recalc(args) => commands::recalc::run(args, cli.human),
        Commands::Validate(args) => commands::validate::run(args, cli.human),
        Commands::Doctor(args) => commands::doctor::run(args, cli.human),
        Commands::Template(args) => commands::template::run(args, cli.human),
        Commands::Schema(args) => commands::schema::run(args, cli.human),
    }
}

fn make_error_envelope<T>(error: XliError) -> ResponseEnvelope<T>
where
    T: Serialize + JsonSchema,
{
    ResponseEnvelope {
        status: Status::Error,
        command: "cli".to_string(),
        input: None,
        output: None,
        commit_mode: CommitMode::None,
        fingerprint_before: None,
        fingerprint_after: None,
        needs_recalc: false,
        stats: CommitStats::default(),
        warnings: Vec::new(),
        errors: vec![error],
        suggested_repairs: Vec::new(),
    }
}
