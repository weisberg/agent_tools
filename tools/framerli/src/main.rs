#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use chrono::{SecondsFormat, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

const DEFAULT_LIMIT: u32 = 50;

fn main() {
    let started = Instant::now();
    let cli = Cli::parse();
    let ctx = Context::new(&cli, started);

    let response = match dispatch(&ctx, cli.command) {
        Ok(value) => Response::ok(value, ctx.meta(None)),
        Err(err) => {
            let exit = err.exit_code();
            let response = Response::err(err, ctx.meta(None));
            print_response(&ctx, &response);
            process::exit(exit);
        }
    };

    print_response(&ctx, &response);
}

#[derive(Debug, Parser)]
#[command(
    name = "framerli",
    version,
    about = "Agent-native Framer Server API control-plane CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format. Defaults to json when stdout is not a TTY.
    #[arg(long, global = true, value_enum)]
    output: Option<OutputFormat>,

    /// Force JSON output.
    #[arg(long, global = true)]
    json: bool,

    /// Emit newline-delimited JSON for streamable commands.
    #[arg(long, global = true)]
    jsonl: bool,

    /// Never prompt. Missing required inputs fail immediately.
    #[arg(long, global = true)]
    non_interactive: bool,

    /// Active Framer profile.
    #[arg(long, global = true, env = "FRAMERLI_PROFILE")]
    profile: Option<String>,

    /// Project URL override.
    #[arg(long, global = true, env = "FRAMERLI_PROJECT")]
    project: Option<String>,

    /// Use a config/state root instead of ~/.config/framerli.
    #[arg(long, global = true)]
    home: Option<PathBuf>,

    /// Explicit dry-run/plan mode for mutating commands.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Execute writes that otherwise require confirmation.
    #[arg(long, global = true)]
    yes: bool,

    /// Include timing metadata.
    #[arg(long, global = true)]
    time: bool,

    /// Disable append-only audit logging for mutating commands.
    #[arg(long, global = true)]
    no_audit: bool,

    /// Increase verbosity.
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Json,
    Jsonl,
    Human,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Authentication and credential checks.
    #[command(subcommand)]
    Auth(AuthCommand),
    /// Project profile and project-level operations.
    #[command(subcommand)]
    Project(ProjectCommand),
    /// CMS collections, fields, items, import/export, schema, and sync.
    #[command(subcommand)]
    Cms(CmsCommand),
    /// The "git status" of a Framer project.
    Status,
    /// Changed contributor information.
    Contributors(ContributorsArgs),
    /// Publish a preview deployment.
    Publish(PublishArgs),
    /// Promote a deployment or rollback via locally tracked history.
    #[command(subcommand)]
    Deploy(DeployCommand),
    /// List known deployments made via framerli.
    #[command(subcommand)]
    Deployments(DeploymentsCommand),
    /// Canvas/node operations.
    #[command(subcommand)]
    Node(NodeCommand),
    /// Site-wide text operations.
    #[command(subcommand)]
    Text(TextCommand),
    /// Code file operations.
    #[command(subcommand)]
    Code(CodeCommand),
    /// Asset operations.
    #[command(subcommand)]
    Assets(AssetsCommand),
    /// Style operations.
    #[command(subcommand)]
    Styles(StylesCommand),
    /// Font operations.
    #[command(subcommand)]
    Fonts(FontsCommand),
    /// Localization operations.
    #[command(name = "i18n")]
    #[command(subcommand)]
    I18n(I18nCommand),
    /// Redirect operations.
    #[command(subcommand)]
    Redirects(RedirectsCommand),
    /// Custom code injection operations.
    #[command(name = "custom-code")]
    #[command(subcommand)]
    CustomCode(CustomCodeCommand),
    /// Declarative site.yaml plan.
    Plan(FileArgs),
    /// Declarative site.yaml apply.
    Apply(ApplyArgs),
    /// Declarative site.yaml diff.
    Diff(FileArgs),
    /// Start MCP stdio server.
    Mcp,
    /// Warm-connection daemon management.
    #[command(subcommand)]
    Daemon(DaemonCommand),
    /// Execute an arbitrary framer-api script through the future bridge.
    Exec(ExecArgs),
    /// Compact project summary for agents.
    Introspect(IntrospectArgs),
    /// Print the command tree as machine-readable schema.
    Tools,
    /// Explain one command's args, output shape, and side effects.
    Explain(ExplainArgs),
    /// Explicit session bracketing.
    #[command(subcommand)]
    Session(SessionCommand),
    /// Record command/output sessions.
    Record { file: PathBuf },
    /// Replay command/output sessions.
    Replay { file: PathBuf },
    /// Current user.
    Whoami,
    /// Permission preflight for an SDK method.
    Can { method: String },
    /// Health checks and runtime diagnostics.
    Doctor,
}

#[derive(Debug, Subcommand)]
enum AuthCommand {
    /// Store an API key for a profile. Use stdin by default.
    Login(AuthLoginArgs),
    /// List stored credential profiles. Secrets are redacted.
    List,
    /// Remove a stored credential.
    Remove { profile: String },
    /// Verify credential by calling project info through the bridge.
    Test,
}

#[derive(Debug, Args)]
struct AuthLoginArgs {
    #[arg(long)]
    profile: Option<String>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    key_stdin: bool,
    #[arg(long)]
    key_env: Option<String>,
    #[arg(long)]
    allow_plaintext: bool,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Set active profile metadata.
    Use { profile_or_url: String },
    /// Project information.
    Info,
    /// Scan for oversized assets, collection limits, and style violations.
    Audit,
}

#[derive(Debug, Subcommand)]
enum CmsCommand {
    #[command(subcommand)]
    Collections(CmsCollectionsCommand),
    #[command(subcommand)]
    Collection(CmsCollectionCommand),
    #[command(subcommand)]
    Fields(CmsFieldsCommand),
    #[command(subcommand)]
    Items(CmsItemsCommand),
    Import(CmsImportArgs),
    Export(CmsExportArgs),
    #[command(subcommand)]
    Schema(CmsSchemaCommand),
    Sync(CmsSyncArgs),
}

#[derive(Debug, Subcommand)]
enum CmsCollectionsCommand {
    List,
}

#[derive(Debug, Subcommand)]
enum CmsCollectionCommand {
    Show { slug: String },
}

#[derive(Debug, Subcommand)]
enum CmsFieldsCommand {
    List {
        collection: String,
    },
    Add(CmsFieldAddArgs),
    Remove {
        collection: String,
        field_id: String,
    },
    Reorder(OrderArgs),
}

#[derive(Debug, Args)]
struct CmsFieldAddArgs {
    collection: String,
    #[arg(long)]
    name: String,
    #[arg(long, value_enum)]
    r#type: FieldType,
    #[arg(long, value_delimiter = ',')]
    cases: Vec<String>,
}

#[derive(Clone, Debug, ValueEnum)]
enum FieldType {
    String,
    Number,
    Boolean,
    FormattedText,
    Image,
    File,
    Color,
    Date,
    Enum,
    Reference,
}

#[derive(Debug, Args)]
struct OrderArgs {
    collection: Option<String>,
    #[arg(long, value_delimiter = ',')]
    order: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum CmsItemsCommand {
    List(CmsItemsListArgs),
    Get {
        collection: String,
        id_or_slug: String,
    },
    Add(CmsItemsAddArgs),
    Remove(CmsItemsRemoveArgs),
    Reorder(OrderArgs),
}

#[derive(Debug, Args)]
struct CmsItemsListArgs {
    collection: String,
    #[arg(long)]
    r#where: Option<String>,
    #[arg(long, default_value_t = DEFAULT_LIMIT)]
    limit: u32,
    #[arg(long)]
    cursor: Option<String>,
}

#[derive(Debug, Args)]
struct CmsItemsAddArgs {
    collection: String,
    #[arg(long)]
    file: Option<PathBuf>,
    #[arg(long)]
    update: bool,
    #[arg(long)]
    if_not_exists: bool,
}

#[derive(Debug, Args)]
struct CmsItemsRemoveArgs {
    collection: String,
    ids: Vec<String>,
}

#[derive(Debug, Args)]
struct CmsImportArgs {
    collection: String,
    #[arg(long, value_enum)]
    from: ImportFormat,
    #[arg(long)]
    map: Option<PathBuf>,
    #[arg(long)]
    file: Option<PathBuf>,
}

#[derive(Clone, Debug, ValueEnum)]
enum ImportFormat {
    Csv,
    Json,
    Ndjson,
    MarkdownDir,
    Rss,
}

#[derive(Debug, Args)]
struct CmsExportArgs {
    collection: String,
    #[arg(long, value_enum, default_value = "json")]
    format: ExportFormat,
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Clone, Debug, ValueEnum)]
enum ExportFormat {
    Csv,
    Json,
    Ndjson,
    MarkdownDir,
}

#[derive(Debug, Subcommand)]
enum CmsSchemaCommand {
    Dump { collection: String },
    Apply(FileArgs),
    Diff(FileArgs),
}

#[derive(Debug, Args)]
struct CmsSyncArgs {
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    watch: bool,
}

#[derive(Debug, Args)]
struct ContributorsArgs {
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
}

#[derive(Debug, Args)]
struct PublishArgs {
    #[arg(long)]
    promote: bool,
    #[arg(long)]
    require_approval: bool,
}

#[derive(Debug, Subcommand)]
enum DeployCommand {
    /// Promote a specific deployment to production.
    Promote { deployment_id: String },
    /// Promote the previous known-good production deployment.
    Rollback,
}

#[derive(Debug, Subcommand)]
enum DeploymentsCommand {
    List,
}

#[derive(Debug, Subcommand)]
enum NodeCommand {
    Get {
        id: String,
    },
    Tree(NodeTreeArgs),
    Find(NodeFindArgs),
    Set(NodeSetArgs),
    Clone {
        id: String,
        #[arg(long)]
        parent: Option<String>,
    },
    Remove {
        id: String,
    },
}

#[derive(Debug, Args)]
struct NodeTreeArgs {
    #[arg(long)]
    depth: Option<u32>,
    #[arg(long)]
    root: Option<String>,
}

#[derive(Debug, Args)]
struct NodeFindArgs {
    #[arg(long)]
    r#type: Option<String>,
    #[arg(long)]
    r#where: Option<String>,
}

#[derive(Debug, Args)]
struct NodeSetArgs {
    id: String,
    #[arg(long)]
    attrs: String,
}

#[derive(Debug, Subcommand)]
enum TextCommand {
    Search(TextSearchArgs),
    Replace(TextReplaceArgs),
    List(TextListArgs),
}

#[derive(Debug, Args)]
struct TextSearchArgs {
    pattern: String,
    #[arg(long)]
    regex: bool,
    #[arg(long)]
    page: Option<String>,
}

#[derive(Debug, Args)]
struct TextReplaceArgs {
    #[arg(long)]
    from: String,
    #[arg(long)]
    to: String,
    #[arg(long)]
    regex: bool,
}

#[derive(Debug, Args)]
struct TextListArgs {
    #[arg(long)]
    page: Option<String>,
}

#[derive(Debug, Subcommand)]
enum CodeCommand {
    List,
    Cat { id: String },
    Write(CodeWriteArgs),
    Rename { id: String, new_name: String },
    Remove { id: String },
    Versions { id: String },
    Typecheck { id: String },
    Lint { id: String },
    Pull { dir: PathBuf },
    Push(CodePushArgs),
}

#[derive(Debug, Args)]
struct CodeWriteArgs {
    id_or_name: String,
    #[arg(long)]
    file: PathBuf,
    #[arg(long)]
    create: bool,
}

#[derive(Debug, Args)]
struct CodePushArgs {
    dir: PathBuf,
    #[arg(long)]
    watch: bool,
}

#[derive(Debug, Subcommand)]
enum AssetsCommand {
    Upload(AssetUploadArgs),
    #[command(subcommand)]
    Svg(AssetSvgCommand),
}

#[derive(Debug, Args)]
struct AssetUploadArgs {
    path: Option<PathBuf>,
    #[arg(long)]
    dir: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "auto")]
    resolution: AssetResolution,
}

#[derive(Clone, Debug, ValueEnum)]
enum AssetResolution {
    Lossless,
    Full,
    Large,
    Medium,
    Small,
    Auto,
}

#[derive(Debug, Subcommand)]
enum AssetSvgCommand {
    Add { file: PathBuf },
}

#[derive(Debug, Subcommand)]
enum StylesCommand {
    #[command(subcommand)]
    Colors(StyleColorCommand),
    #[command(subcommand)]
    Text(StyleTextCommand),
    Export {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Apply(FileArgs),
}

#[derive(Debug, Subcommand)]
enum StyleColorCommand {
    List,
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        rgba: Option<String>,
        #[arg(long)]
        hex: Option<String>,
    },
    Remove {
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum StyleTextCommand {
    List,
    Create {
        #[arg(long)]
        name: String,
    },
    Remove {
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum FontsCommand {
    List,
    Get(FontGetArgs),
}

#[derive(Debug, Args)]
struct FontGetArgs {
    family: String,
    #[arg(long)]
    weight: Option<u16>,
    #[arg(long)]
    style: Option<String>,
}

#[derive(Debug, Subcommand)]
enum I18nCommand {
    #[command(subcommand)]
    Locales(I18nLocalesCommand),
    #[command(subcommand)]
    Groups(I18nGroupsCommand),
    Export(I18nExportArgs),
    Import(FileArgs),
    Diff {
        #[arg(long)]
        locale: String,
    },
}

#[derive(Debug, Subcommand)]
enum I18nLocalesCommand {
    List,
}

#[derive(Debug, Subcommand)]
enum I18nGroupsCommand {
    List,
}

#[derive(Debug, Args)]
struct I18nExportArgs {
    #[arg(long)]
    locale: String,
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum RedirectsCommand {
    List,
    Add(RedirectAddArgs),
    Remove {
        ids: Vec<String>,
    },
    Reorder {
        #[arg(long, value_delimiter = ',')]
        order: Vec<String>,
    },
    Import {
        #[arg(long)]
        file: PathBuf,
    },
}

#[derive(Debug, Args)]
struct RedirectAddArgs {
    #[arg(long)]
    from: String,
    #[arg(long)]
    to: String,
    #[arg(long)]
    expand_locales: bool,
}

#[derive(Debug, Subcommand)]
enum CustomCodeCommand {
    Get {
        #[arg(long, value_enum)]
        location: CodeLocation,
    },
    Set {
        #[arg(long, value_enum)]
        location: CodeLocation,
        #[arg(long)]
        file: PathBuf,
    },
    Clear {
        #[arg(long, value_enum)]
        location: CodeLocation,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum CodeLocation {
    HeadStart,
    HeadEnd,
    BodyStart,
    BodyEnd,
}

#[derive(Debug, Args)]
struct FileArgs {
    #[arg(short, long)]
    file: PathBuf,
}

#[derive(Debug, Args)]
struct ApplyArgs {
    #[arg(short, long)]
    file: PathBuf,
    #[arg(long)]
    auto_approve: bool,
}

#[derive(Debug, Subcommand)]
enum DaemonCommand {
    Start,
    Stop,
    Status,
}

#[derive(Debug, Args)]
struct ExecArgs {
    script: PathBuf,
    #[arg(last = true)]
    args: Vec<String>,
}

#[derive(Debug, Args)]
struct IntrospectArgs {
    #[arg(long, value_enum, default_value = "shallow")]
    depth: IntrospectionDepth,
}

#[derive(Clone, Debug, ValueEnum)]
enum IntrospectionDepth {
    Shallow,
    Full,
}

#[derive(Debug, Args)]
struct ExplainArgs {
    command: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum SessionCommand {
    Begin,
    End,
}

#[derive(Debug)]
struct Context {
    profile: String,
    project: Option<String>,
    started: Instant,
    home: PathBuf,
    output: OutputFormat,
    dry_run: bool,
    yes: bool,
    time: bool,
    no_audit: bool,
}

impl Context {
    fn new(cli: &Cli, started: Instant) -> Self {
        let home = cli
            .home
            .clone()
            .or_else(|| dirs::config_dir().map(|p| p.join("framerli")))
            .unwrap_or_else(|| PathBuf::from(".framerli"));
        let config = Config::load(&home);
        let profile = cli
            .profile
            .clone()
            .or(config.default_profile)
            .unwrap_or_else(|| "default".to_string());
        let project = cli.project.clone().or_else(|| {
            config
                .profiles
                .get(&profile)
                .and_then(|p| p.project.clone())
        });
        let output = if cli.jsonl {
            OutputFormat::Jsonl
        } else if cli.json {
            OutputFormat::Json
        } else {
            cli.output.unwrap_or(OutputFormat::Json)
        };

        Self {
            profile,
            project,
            started,
            home,
            output,
            dry_run: cli.dry_run,
            yes: cli.yes,
            time: cli.time,
            no_audit: cli.no_audit,
        }
    }

    fn meta(&self, sdk_method: Option<&str>) -> Meta {
        Meta {
            ms: self.started.elapsed().as_millis(),
            sdk_method: sdk_method.map(str::to_string),
            profile: self.profile.clone(),
            project: self.project.clone(),
            dry_run: self.dry_run,
            generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            timing_enabled: self.time,
        }
    }

    fn state_dir(&self) -> PathBuf {
        self.home.join("state")
    }
}

#[derive(Debug, Default, Deserialize)]
struct Config {
    default_profile: Option<String>,
    #[serde(default, rename = "profile")]
    profiles: std::collections::BTreeMap<String, ProfileConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct ProfileConfig {
    project: Option<String>,
}

impl Config {
    fn load(home: &Path) -> Self {
        let path = discover_config(home);
        path.and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }
}

fn discover_config(home: &Path) -> Option<PathBuf> {
    let mut dir = env::current_dir().ok();
    while let Some(current) = dir {
        let candidate = current.join("framerli.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        dir = current.parent().map(Path::to_path_buf);
    }
    let global = home.join("config.toml");
    global.exists().then_some(global)
}

fn dispatch(ctx: &Context, command: Commands) -> Result<Value, CliError> {
    match command {
        Commands::Tools => Ok(command_tree()),
        Commands::Explain(args) => Ok(explain_command(&args.command)),
        Commands::Auth(cmd) => handle_auth(ctx, cmd),
        Commands::Project(ProjectCommand::Use { profile_or_url }) => Ok(json!({
            "selected": profile_or_url,
            "scope": "project",
            "note": "Profile persistence is scaffolded; write support will land with credential storage."
        })),
        Commands::Deployments(DeploymentsCommand::List) => Ok(json!({"items": [], "count": 0})),
        Commands::Daemon(DaemonCommand::Status) => Ok(json!({"running": false, "mode": "not_started", "socket": null})),
        Commands::Session(SessionCommand::Begin) => Ok(json!({"session": new_token("sess"), "status": "begun"})),
        Commands::Session(SessionCommand::End) => Ok(json!({"status": "ended"})),
        Commands::Doctor => Ok(json!({
            "cli": {"name": "framerli", "version": env!("CARGO_PKG_VERSION")},
            "config_home": ctx.home,
            "profile": ctx.profile,
            "project_configured": ctx.project.is_some(),
            "bridge": bridge_status()
        })),
        Commands::Mcp => Err(CliError::BridgeUnavailable {
            command: "mcp".to_string(),
            hint: "The Rust CLI command surface is ready; MCP requires the Node framer-api bridge/sidecar."
                .to_string(),
        }),
        Commands::Daemon(_) => Err(CliError::BridgeUnavailable {
            command: "daemon".to_string(),
            hint: "Daemon lifecycle is part of the planned warm WebSocket sidecar.".to_string(),
        }),
        Commands::Record { file } | Commands::Replay { file } => Ok(json!({
            "file": file,
            "status": "planned",
            "dry_run": true,
            "note": "Record/replay schema is reserved for the deterministic agent-eval harness."
        })),
        other => handle_api_command(ctx, other),
    }
}

fn handle_auth(ctx: &Context, cmd: AuthCommand) -> Result<Value, CliError> {
    match cmd {
        AuthCommand::Login(args) => {
            let key_source = if let Some(name) = args.key_env {
                if env::var(&name).is_err() {
                    return Err(CliError::Usage {
                        message: format!("environment variable {name} is not set"),
                        hint: "Set the variable or pass --key-stdin.".to_string(),
                    });
                }
                format!("env:{name}")
            } else if args.key_stdin {
                let mut key = String::new();
                io::stdin().read_to_string(&mut key)?;
                if key.trim().is_empty() {
                    return Err(CliError::AuthMissing {
                        hint: "Pipe an API key on stdin or use --key-env.".to_string(),
                    });
                }
                if args.allow_plaintext {
                    "stdin:plaintext-not-persisted".to_string()
                } else {
                    "stdin:keychain-pending".to_string()
                }
            } else {
                return Err(CliError::AuthMissing {
                    hint: "Use --key-stdin or --key-env NAME. Interactive prompting is intentionally not implemented."
                        .to_string(),
                });
            };

            Ok(json!({
                "profile": args.profile.unwrap_or_else(|| ctx.profile.clone()),
                "project": args.project.or_else(|| ctx.project.clone()),
                "key_source": key_source,
                "stored": false,
                "note": "Credential capture is validated, but persistent keychain storage is reserved for the bridge phase."
            }))
        }
        AuthCommand::List => Ok(json!({"profiles": [], "count": 0, "redacted": true})),
        AuthCommand::Remove { profile } => Ok(
            json!({"profile": profile, "removed": false, "reason": "no local credential store yet"}),
        ),
        AuthCommand::Test => Err(CliError::BridgeUnavailable {
            command: "auth test".to_string(),
            hint: "Credential testing requires the Node framer-api bridge.".to_string(),
        }),
    }
}

fn handle_api_command(ctx: &Context, command: Commands) -> Result<Value, CliError> {
    if is_mutating(&command) && requires_confirmation(&command) && !ctx.yes && !ctx.dry_run {
        return Err(CliError::ApprovalRequired {
            command: command_name(&command),
            hint: "Rerun with --dry-run to preview or --yes to apply.".to_string(),
        });
    }

    let plan = json!({
        "command": command_name(&command),
        "sdk_methods": sdk_methods_for(&command),
        "mutating": is_mutating(&command),
        "dry_run": ctx.dry_run || is_mutating(&command),
        "bridge": bridge_status(),
        "status": "planned",
        "note": "Rust command contract is implemented. Live Framer calls require the official Node framer-api bridge/daemon."
    });

    if is_live_read(&command) && !ctx.dry_run {
        return Err(CliError::BridgeUnavailable {
            command: command_name(&command),
            hint: "Set up the future Node bridge to execute live Framer Server API reads."
                .to_string(),
        });
    }

    append_audit(ctx, &command, &plan)?;
    Ok(plan)
}

fn print_response(ctx: &Context, response: &Response) {
    match ctx.output {
        OutputFormat::Json | OutputFormat::Jsonl => {
            println!(
                "{}",
                serde_json::to_string_pretty(response).expect("serializable response")
            );
        }
        OutputFormat::Human => {
            if response.ok {
                println!("ok");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&response.data).expect("serializable data")
                );
            } else if let Some(err) = &response.error {
                eprintln!("{}: {}", err.code, err.message);
                if let Some(hint) = &err.hint {
                    eprintln!("hint: {hint}");
                }
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorBody>,
    meta: Meta,
}

impl Response {
    fn ok(data: Value, meta: Meta) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
            meta,
        }
    }

    fn err(error: CliError, meta: Meta) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(error.into()),
            meta,
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    hint: Option<String>,
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    sdk_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Meta {
    ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    sdk_method: Option<String>,
    profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    dry_run: bool,
    generated_at: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    timing_enabled: bool,
}

#[derive(Debug, Error)]
enum CliError {
    #[error("{message}")]
    Usage { message: String, hint: String },
    #[error("No Framer API credential is configured")]
    AuthMissing { hint: String },
    #[error("Approval required for {command}")]
    ApprovalRequired { command: String, hint: String },
    #[error("Bridge unavailable for {command}")]
    BridgeUnavailable { command: String, hint: String },
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

impl CliError {
    fn exit_code(&self) -> i32 {
        match self {
            Self::Usage { .. } => 2,
            Self::AuthMissing { .. } => 3,
            Self::ApprovalRequired { .. } => 5,
            Self::BridgeUnavailable { .. } => 10,
            Self::Io(_) => 1,
        }
    }
}

impl From<CliError> for ErrorBody {
    fn from(value: CliError) -> Self {
        match value {
            CliError::Usage { message, hint } => Self {
                code: "E_USAGE",
                message,
                hint: Some(hint),
                retryable: false,
                sdk_method: None,
                details: None,
            },
            CliError::AuthMissing { hint } => Self {
                code: "E_AUTH_MISSING",
                message: "No Framer API credential is configured.".to_string(),
                hint: Some(hint),
                retryable: false,
                sdk_method: None,
                details: None,
            },
            CliError::ApprovalRequired { command, hint } => Self {
                code: "E_APPROVAL_REQUIRED",
                message: format!("Command '{command}' requires approval."),
                hint: Some(hint),
                retryable: true,
                sdk_method: None,
                details: Some(
                    json!({ "command": command, "pending_action": new_token("approval") }),
                ),
            },
            CliError::BridgeUnavailable { command, hint } => Self {
                code: "E_BRIDGE_UNAVAILABLE",
                message: format!("Command '{command}' requires the Framer Server API bridge."),
                hint: Some(hint),
                retryable: false,
                sdk_method: None,
                details: Some(bridge_status()),
            },
            CliError::Io(err) => Self {
                code: "E_IO",
                message: err.to_string(),
                hint: None,
                retryable: false,
                sdk_method: None,
                details: None,
            },
        }
    }
}

fn command_tree() -> Value {
    json!({
        "name": "framerli",
        "version": env!("CARGO_PKG_VERSION"),
        "output_contract": {
            "success": {"ok": true, "data": {}, "meta": {}},
            "error": {"ok": false, "error": {"code": "E_*", "message": "", "hint": "", "retryable": false}, "meta": {}}
        },
        "groups": [
            {"name": "auth", "commands": ["login", "list", "remove", "test"]},
            {"name": "project", "commands": ["use", "info", "audit"]},
            {"name": "cms", "commands": ["collections list", "collection show", "fields list/add/remove/reorder", "items list/get/add/remove/reorder", "import", "export", "schema dump/apply/diff", "sync"]},
            {"name": "publish", "commands": ["publish", "deploy promote", "deploy rollback", "deployments list", "status", "contributors"]},
            {"name": "canvas", "commands": ["node get/tree/find/set/clone/remove", "text search/replace/list"]},
            {"name": "code", "commands": ["list", "cat", "write", "rename", "remove", "versions", "typecheck", "lint", "pull", "push"]},
            {"name": "assets", "commands": ["upload", "svg add"]},
            {"name": "styles", "commands": ["colors list/create/remove", "text list/create/remove", "export", "apply"]},
            {"name": "fonts", "commands": ["list", "get"]},
            {"name": "i18n", "commands": ["locales list", "groups list", "export", "import", "diff"]},
            {"name": "redirects", "commands": ["list", "add", "remove", "reorder", "import"]},
            {"name": "custom-code", "commands": ["get", "set", "clear"]},
            {"name": "declarative", "commands": ["plan", "apply", "diff"]},
            {"name": "agent", "commands": ["mcp", "daemon start/stop/status", "exec", "introspect", "tools", "explain", "session begin/end", "record", "replay"]}
        ]
    })
}

fn explain_command(parts: &[String]) -> Value {
    let key = parts.join(" ");
    let side_effects = matches!(
        key.as_str(),
        "publish" | "deploy" | "cms items add" | "cms items remove" | "text replace" | "apply"
    );
    json!({
        "command": key,
        "side_effects": side_effects,
        "supports_dry_run": side_effects,
        "output_shape": {"ok": "boolean", "data": "object", "meta": "object"},
        "error_shape": {"ok": false, "error": {"code": "stable string", "message": "string", "hint": "string|null", "retryable": "boolean"}},
        "notes": "Run `framerli tools` for the full machine-readable command catalog."
    })
}

fn bridge_status() -> Value {
    json!({
        "kind": "node-framer-api",
        "available": false,
        "env": {
            "FRAMER_API_KEY": env::var("FRAMER_API_KEY").is_ok(),
            "FRAMERLI_BRIDGE": env::var("FRAMERLI_BRIDGE").ok()
        }
    })
}

fn append_audit(ctx: &Context, command: &Commands, payload: &Value) -> Result<(), CliError> {
    if ctx.no_audit || !is_mutating(command) {
        return Ok(());
    }
    fs::create_dir_all(ctx.state_dir())?;
    let path = ctx.state_dir().join("audit.ndjson");
    let line = json!({
        "ts": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        "profile": ctx.profile,
        "cmd": command_name(command),
        "result": "planned",
        "dry_run": ctx.dry_run,
        "payload": payload
    });
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(
        file,
        "{}",
        serde_json::to_string(&line).expect("audit line")
    )?;
    Ok(())
}

fn command_name(command: &Commands) -> String {
    match command {
        Commands::Auth(_) => "auth".into(),
        Commands::Project(ProjectCommand::Use { .. }) => "project use".into(),
        Commands::Project(ProjectCommand::Info) => "project info".into(),
        Commands::Project(ProjectCommand::Audit) => "project audit".into(),
        Commands::Cms(_) => "cms".into(),
        Commands::Status => "status".into(),
        Commands::Contributors(_) => "contributors".into(),
        Commands::Publish(_) => "publish".into(),
        Commands::Deploy(_) => "deploy".into(),
        Commands::Deployments(_) => "deployments list".into(),
        Commands::Node(_) => "node".into(),
        Commands::Text(TextCommand::Replace(_)) => "text replace".into(),
        Commands::Text(_) => "text".into(),
        Commands::Code(_) => "code".into(),
        Commands::Assets(_) => "assets".into(),
        Commands::Styles(_) => "styles".into(),
        Commands::Fonts(_) => "fonts".into(),
        Commands::I18n(_) => "i18n".into(),
        Commands::Redirects(_) => "redirects".into(),
        Commands::CustomCode(_) => "custom-code".into(),
        Commands::Plan(_) => "plan".into(),
        Commands::Apply(_) => "apply".into(),
        Commands::Diff(_) => "diff".into(),
        Commands::Mcp => "mcp".into(),
        Commands::Daemon(_) => "daemon".into(),
        Commands::Exec(_) => "exec".into(),
        Commands::Introspect(_) => "introspect".into(),
        Commands::Tools => "tools".into(),
        Commands::Explain(_) => "explain".into(),
        Commands::Session(_) => "session".into(),
        Commands::Record { .. } => "record".into(),
        Commands::Replay { .. } => "replay".into(),
        Commands::Whoami => "whoami".into(),
        Commands::Can { .. } => "can".into(),
        Commands::Doctor => "doctor".into(),
    }
}

fn is_live_read(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Project(ProjectCommand::Info)
            | Commands::Project(ProjectCommand::Audit)
            | Commands::Cms(_)
            | Commands::Status
            | Commands::Contributors(_)
            | Commands::Node(_)
            | Commands::Text(_)
            | Commands::Code(_)
            | Commands::Assets(_)
            | Commands::Styles(_)
            | Commands::Fonts(_)
            | Commands::I18n(_)
            | Commands::Redirects(_)
            | Commands::CustomCode(_)
            | Commands::Introspect(_)
            | Commands::Whoami
            | Commands::Can { .. }
    )
}

fn is_mutating(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Publish(_)
            | Commands::Deploy(_)
            | Commands::Cms(CmsCommand::Fields(CmsFieldsCommand::Add(_)))
            | Commands::Cms(CmsCommand::Fields(CmsFieldsCommand::Remove { .. }))
            | Commands::Cms(CmsCommand::Fields(CmsFieldsCommand::Reorder(_)))
            | Commands::Cms(CmsCommand::Items(CmsItemsCommand::Add(_)))
            | Commands::Cms(CmsCommand::Items(CmsItemsCommand::Remove(_)))
            | Commands::Cms(CmsCommand::Items(CmsItemsCommand::Reorder(_)))
            | Commands::Cms(CmsCommand::Import(_))
            | Commands::Cms(CmsCommand::Schema(CmsSchemaCommand::Apply(_)))
            | Commands::Cms(CmsCommand::Sync(_))
            | Commands::Node(NodeCommand::Set(_))
            | Commands::Node(NodeCommand::Clone { .. })
            | Commands::Node(NodeCommand::Remove { .. })
            | Commands::Text(TextCommand::Replace(_))
            | Commands::Code(CodeCommand::Write(_))
            | Commands::Code(CodeCommand::Rename { .. })
            | Commands::Code(CodeCommand::Remove { .. })
            | Commands::Code(CodeCommand::Push(_))
            | Commands::Assets(AssetsCommand::Upload(_))
            | Commands::Assets(AssetsCommand::Svg(_))
            | Commands::Styles(StylesCommand::Colors(StyleColorCommand::Create { .. }))
            | Commands::Styles(StylesCommand::Colors(StyleColorCommand::Remove { .. }))
            | Commands::Styles(StylesCommand::Text(StyleTextCommand::Create { .. }))
            | Commands::Styles(StylesCommand::Text(StyleTextCommand::Remove { .. }))
            | Commands::Styles(StylesCommand::Apply(_))
            | Commands::I18n(I18nCommand::Import(_))
            | Commands::Redirects(RedirectsCommand::Add(_))
            | Commands::Redirects(RedirectsCommand::Remove { .. })
            | Commands::Redirects(RedirectsCommand::Reorder { .. })
            | Commands::Redirects(RedirectsCommand::Import { .. })
            | Commands::CustomCode(CustomCodeCommand::Set { .. })
            | Commands::CustomCode(CustomCodeCommand::Clear { .. })
            | Commands::Apply(_)
    )
}

fn requires_confirmation(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Deploy(_)
            | Commands::Cms(CmsCommand::Items(CmsItemsCommand::Remove(_)))
            | Commands::Node(NodeCommand::Remove { .. })
            | Commands::Code(CodeCommand::Remove { .. })
            | Commands::Redirects(RedirectsCommand::Remove { .. })
            | Commands::CustomCode(CustomCodeCommand::Clear { .. })
            | Commands::Apply(_)
    )
}

fn sdk_methods_for(command: &Commands) -> Vec<&'static str> {
    match command {
        Commands::Project(ProjectCommand::Info) => vec!["getProjectInfo", "getPublishInfo"],
        Commands::Status => vec!["getChangedPaths"],
        Commands::Contributors(_) => vec!["getChangeContributors"],
        Commands::Publish(_) => vec!["publish"],
        Commands::Deploy(_) => vec!["deploy"],
        Commands::Cms(CmsCommand::Collections(_)) => {
            vec!["getCollections", "getManagedCollections"]
        }
        Commands::Cms(CmsCommand::Items(CmsItemsCommand::List(_))) => vec!["collection.getItems"],
        Commands::Cms(CmsCommand::Items(CmsItemsCommand::Add(_))) => vec!["collection.addItems"],
        Commands::Cms(CmsCommand::Items(CmsItemsCommand::Remove(_))) => {
            vec!["collection.removeItems"]
        }
        Commands::Cms(CmsCommand::Fields(_)) | Commands::Cms(CmsCommand::Schema(_)) => {
            vec!["managedCollection.setFields"]
        }
        Commands::Node(NodeCommand::Get { .. }) => vec!["getNode"],
        Commands::Node(NodeCommand::Set(_)) => vec!["setAttributes"],
        Commands::Node(NodeCommand::Find(_)) => vec!["getNodesWithType", "getNodesWithAttribute"],
        Commands::Code(_) => vec!["getCodeFiles", "codeFile.setFileContent", "createCodeFile"],
        Commands::Assets(_) => vec!["uploadImage", "uploadFile"],
        Commands::I18n(_) => vec!["getLocales", "getLocalizationGroups", "setLocalizationData"],
        Commands::Redirects(_) => vec!["addRedirects"],
        Commands::CustomCode(_) => vec!["setCustomCode"],
        Commands::Whoami => vec!["getCurrentUser"],
        Commands::Can { .. } => vec!["isAllowedTo"],
        _ => vec![],
    }
}

fn new_token(prefix: &str) -> String {
    format!(
        "{}_{}",
        prefix,
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    )
}
