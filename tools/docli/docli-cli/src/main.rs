use clap::{Parser, Subcommand};

mod commands;
mod envelope;

use commands::{
    convert, create, diff, doctor, edit, extract, finalize, inspect, kb, merge, ooxml, read,
    review, run, schema, template, validate,
};

#[derive(Parser)]
#[command(name = "docli", version, about = "DOCX document intelligence CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Output format: json (default), yaml, text
    #[arg(long, global = true, default_value = "json")]
    format: String,
    /// Pretty-print JSON output
    #[arg(long, global = true)]
    pretty: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect a DOCX file: index structure and extract metadata
    Inspect(inspect::InspectArgs),
    /// Validate a DOCX file for structural and invariant issues
    Validate(validate::ValidateArgs),
    /// OOXML low-level operations (unpack, pack, query)
    #[command(subcommand)]
    Ooxml(ooxml::OoxmlCommand),
    /// Knowledge base operations (list, get)
    #[command(subcommand)]
    Kb(kb::KbCommand),
    /// Print JSON schemas for core types
    Schema(schema::SchemaArgs),
    /// Check system dependencies and environment health
    Doctor(doctor::DoctorArgs),
    /// Read and render document content
    Read(read::ReadArgs),
    /// Create a new DOCX from a YAML spec
    Create(create::CreateArgs),
    /// Template operations (list, get, render)
    #[command(subcommand)]
    Template(template::TemplateCommand),
    /// Edit operations (replace, insert, delete, find-replace)
    #[command(subcommand)]
    Edit(edit::EditCommand),
    /// Run a batch job file (YAML or JSON)
    Run(run::RunArgs),
    /// Review operations (comment, track-replace, track-insert, track-delete)
    #[command(subcommand)]
    Review(review::ReviewCommand),
    /// Finalize tracked changes (accept, reject, strip)
    #[command(subcommand)]
    Finalize(finalize::FinalizeCommand),
    /// Semantic diff between two DOCX files
    Diff(diff::DiffArgs),
    /// Convert a DOCX to another format
    Convert(convert::ConvertArgs),
    /// Extract content from a DOCX file
    #[command(subcommand)]
    Extract(extract::ExtractCommand),
    /// Merge two DOCX files
    Merge(merge::MergeArgs),
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Inspect(args) => inspect::run(args, &cli.format, cli.pretty),
        Commands::Validate(args) => validate::run(args, &cli.format, cli.pretty),
        Commands::Ooxml(cmd) => ooxml::run(cmd, &cli.format, cli.pretty),
        Commands::Kb(cmd) => kb::run(cmd, &cli.format, cli.pretty),
        Commands::Schema(args) => schema::run(args, &cli.format, cli.pretty),
        Commands::Doctor(args) => doctor::run(args, &cli.format, cli.pretty),
        Commands::Read(args) => read::run(args, &cli.format, cli.pretty),
        Commands::Create(args) => create::run(args, &cli.format, cli.pretty),
        Commands::Template(cmd) => template::run(cmd, &cli.format, cli.pretty),
        Commands::Edit(cmd) => edit::run(cmd, &cli.format, cli.pretty),
        Commands::Run(args) => run::run(args, &cli.format, cli.pretty),
        Commands::Review(cmd) => review::run(cmd, &cli.format, cli.pretty),
        Commands::Finalize(cmd) => finalize::run(cmd, &cli.format, cli.pretty),
        Commands::Diff(args) => diff::run(args, &cli.format, cli.pretty),
        Commands::Convert(args) => convert::run(args, &cli.format, cli.pretty),
        Commands::Extract(cmd) => extract::run(cmd, &cli.format, cli.pretty),
        Commands::Merge(args) => merge::run(args, &cli.format, cli.pretty),
    };

    std::process::exit(exit_code);
}
