use crate::cli::{Cli, Command};
use crate::commands;

pub fn run(cli: Cli) -> Result<String, serde_json::Error> {
    match cli.command {
        Command::Status => serde_json::to_string_pretty(&commands::status::run(cli.dry_run)),

        // TODO(slackli-mvp-phase-1): implement concrete send/reply/history/thread handlers.
        Command::Send => serde_json::to_string_pretty(&commands::send::run(cli.dry_run)),
        Command::Reply => serde_json::to_string_pretty(&commands::reply::run(cli.dry_run)),
        Command::History => serde_json::to_string_pretty(&commands::history::run(cli.dry_run)),
        Command::Thread => serde_json::to_string_pretty(&commands::thread::run(cli.dry_run)),

        // TODO(slackli-mvp-phase-2): config/token storage via keychain + local profiles.
        Command::Auth => serde_json::to_string_pretty(&commands::auth::run(cli.dry_run)),
        Command::Config => serde_json::to_string_pretty(&commands::config::run(cli.dry_run)),

        // TODO(slackli-mvp-phase-3): transport-level rate limiting and retry-after support.
        Command::Update => serde_json::to_string_pretty(&commands::update::run(cli.dry_run)),
        Command::React => serde_json::to_string_pretty(&commands::react::run(cli.dry_run)),

        // TODO(slackli-mvp-phase-4): socket mode event stream + route config.
        Command::Listen => serde_json::to_string_pretty(&commands::listen::run(cli.dry_run)),

        // TODO(slackli-mvp-phase-5): agent orchestration (once/run), approvals, dry-run planning.
        Command::Agent => serde_json::to_string_pretty(&commands::agent::run(cli.dry_run)),
        Command::Approvals => serde_json::to_string_pretty(&commands::approvals::run(cli.dry_run)),

        // TODO(slackli-mvp-phase-6): semantic search primary path + legacy fallback warnings.
        Command::Search => serde_json::to_string_pretty(&commands::search::run(cli.dry_run)),

        // TODO(slackli-mvp-phase-7): expose MCP server mode for local agent interoperability.
        Command::Mcp => serde_json::to_string_pretty(&commands::mcp::run(cli.dry_run)),

        // TODO(slackli-post-mvp): implement remaining surfaces.
        Command::Delete => serde_json::to_string_pretty(&commands::delete::run(cli.dry_run)),
        Command::Upload => serde_json::to_string_pretty(&commands::upload::run(cli.dry_run)),
        Command::Channels => serde_json::to_string_pretty(&commands::channels::run(cli.dry_run)),
        Command::Users => serde_json::to_string_pretty(&commands::users::run(cli.dry_run)),
    }
}
