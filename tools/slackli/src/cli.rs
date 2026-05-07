use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Ndjson,
    Markdown,
}

#[derive(Debug, Parser)]
#[command(name = "slackli", version, about = "Agent-safe Slack CLI")]
pub struct Cli {
    /// Output format. Defaults to JSON; NDJSON is recommended for streams.
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,

    /// Do not perform mutating writes; print proposed actions.
    #[arg(long, global = true)]
    pub dry_run: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show tool health and metadata in JSON.
    Status,
    Send,
    Reply,
    Update,
    Delete,
    React,
    Upload,
    Listen,
    History,
    Thread,
    Search,
    Auth,
    Users,
    Channels,
    Agent,
    Mcp,
    Config,
    Approvals,
}
