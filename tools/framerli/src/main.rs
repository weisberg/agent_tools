#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
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

    /// Use a state root instead of the platform data directory.
    #[arg(long, global = true, env = "FRAMERLI_HOME")]
    home: Option<PathBuf>,

    /// Config file path. Defaults to ~/.config/framerli.yaml.
    #[arg(long, global = true, env = "FRAMERLI_CONFIG")]
    config: Option<PathBuf>,

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

    /// Override the Node framer-api bridge script.
    #[arg(long, global = true, env = "FRAMERLI_BRIDGE")]
    bridge: Option<PathBuf>,

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
    config_path: PathBuf,
    output: OutputFormat,
    dry_run: bool,
    yes: bool,
    time: bool,
    no_audit: bool,
    bridge: Option<PathBuf>,
    key_source: Option<String>,
}

impl Context {
    fn new(cli: &Cli, started: Instant) -> Self {
        let home = cli
            .home
            .clone()
            .or_else(|| env::var_os("FRAMERLI_HOME").map(PathBuf::from))
            .or_else(|| dirs::data_local_dir().map(|p| p.join("framerli")))
            .unwrap_or_else(|| PathBuf::from(".framerli"));
        let config_path = discover_config_path(cli.config.as_deref(), &home);
        let config = Config::load(&config_path, &home);
        let profile = cli
            .profile
            .clone()
            .or_else(|| env::var("FRAMERLI_DEFAULT_PROFILE").ok())
            .or(config.default_profile.clone())
            .unwrap_or_else(|| "default".to_string());
        let profile_config = config.profiles.get(&profile);
        let project = cli
            .project
            .clone()
            .or_else(|| env::var("FRAMERLI_PROJECT").ok())
            .or_else(|| profile_config.and_then(|p| p.project.clone()));
        let key_source = env::var("FRAMERLI_KEY_SOURCE")
            .ok()
            .or_else(|| profile_config.and_then(|p| p.key_source.clone()));
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
            config_path,
            output,
            dry_run: cli.dry_run,
            yes: cli.yes,
            time: cli.time,
            no_audit: cli.no_audit,
            bridge: cli.bridge.clone().or_else(default_bridge_path),
            key_source,
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

#[derive(Debug, Default, Deserialize, Serialize)]
struct Config {
    default_profile: Option<String>,
    #[serde(default, rename = "profile")]
    profiles: std::collections::BTreeMap<String, ProfileConfig>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ProfileConfig {
    project: Option<String>,
    key_source: Option<String>,
}

impl Config {
    fn load(path: &Path, legacy_home: &Path) -> Self {
        read_config(path)
            .or_else(|| legacy_config_path(legacy_home).and_then(|path| read_config(&path)))
            .unwrap_or_default()
    }
}

fn read_config(path: &Path) -> Option<Config> {
    let contents = fs::read_to_string(path).ok()?;
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("yaml" | "yml") => serde_yaml::from_str(&contents).ok(),
        Some("toml") => toml::from_str(&contents).ok(),
        _ => serde_yaml::from_str(&contents)
            .ok()
            .or_else(|| toml::from_str(&contents).ok()),
    }
}

fn discover_config_path(explicit: Option<&Path>, _legacy_home: &Path) -> PathBuf {
    if let Some(path) = explicit {
        return path.to_path_buf();
    }
    if let Some(path) = env::var_os("FRAMERLI_CONFIG").map(PathBuf::from) {
        return path;
    }
    let mut dir = env::current_dir().ok();
    while let Some(current) = dir {
        for filename in ["framerli.yaml", "framerli.yml", "framerli.toml"] {
            let candidate = current.join(filename);
            if candidate.exists() {
                return candidate;
            }
        }
        dir = current.parent().map(Path::to_path_buf);
    }
    global_config_path()
}

fn global_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("framerli.yaml")
}

fn legacy_config_path(home: &Path) -> Option<PathBuf> {
    let path = home.join("config.toml");
    path.exists().then_some(path)
}

fn save_config(path: &Path, config: &Config) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_yaml::to_string(config).map_err(|err| CliError::Usage {
        message: err.to_string(),
        hint: "Could not serialize framerli YAML config.".to_string(),
    })?;
    fs::write(path, contents)?;
    Ok(())
}

fn dispatch(ctx: &Context, command: Commands) -> Result<Value, CliError> {
    match command {
        Commands::Tools => Ok(command_tree()),
        Commands::Explain(args) => Ok(explain_command(&args.command)),
        Commands::Auth(cmd) => handle_auth(ctx, cmd),
        Commands::Project(ProjectCommand::Use { profile_or_url }) => handle_project_use(ctx, profile_or_url),
        Commands::Deployments(DeploymentsCommand::List) => Ok(json!({"items": [], "count": 0})),
        Commands::Daemon(DaemonCommand::Status) => Ok(json!({"running": false, "mode": "not_started", "socket": null})),
        Commands::Session(SessionCommand::Begin) => Ok(json!({"session": new_token("sess"), "status": "begun"})),
        Commands::Session(SessionCommand::End) => Ok(json!({"status": "ended"})),
        Commands::Doctor => Ok(json!({
            "cli": {"name": "framerli", "version": env!("CARGO_PKG_VERSION")},
            "config_home": ctx.home,
            "config_path": ctx.config_path,
            "profile": ctx.profile,
            "project_configured": ctx.project.is_some(),
            "bridge": bridge_status(ctx)
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

            let profile = args.profile.unwrap_or_else(|| ctx.profile.clone());
            let project = args.project.or_else(|| ctx.project.clone());
            let mut saved = false;
            if key_source.starts_with("env:") || project.is_some() {
                let mut config = Config::load(&ctx.config_path, &ctx.home);
                config.default_profile = Some(profile.clone());
                let entry = config.profiles.entry(profile.clone()).or_default();
                if let Some(project) = &project {
                    entry.project = Some(project.clone());
                }
                if key_source.starts_with("env:") {
                    entry.key_source = Some(key_source.clone());
                }
                save_config(&ctx.config_path, &config)?;
                saved = true;
            }

            Ok(json!({
                "profile": profile,
                "project": project,
                "key_source": key_source,
                "stored": saved,
                "note": "Environment key sources are persisted as references only. Plain API key material is not written to disk."
            }))
        }
        AuthCommand::List => Ok(json!({"profiles": [], "count": 0, "redacted": true})),
        AuthCommand::Remove { profile } => Ok(
            json!({"profile": profile, "removed": false, "reason": "no local credential store yet"}),
        ),
        AuthCommand::Test => {
            let request = BridgeRequest {
                version: 1,
                operation: "project.info".to_string(),
                profile: ctx.profile.clone(),
                project: ctx.project.clone(),
                dry_run: false,
                args: json!({}),
            };
            invoke_bridge(ctx, &request)
        }
    }
}

fn handle_project_use(ctx: &Context, profile_or_url: String) -> Result<Value, CliError> {
    let mut config = Config::load(&ctx.config_path, &ctx.home);
    let is_url = profile_or_url.starts_with("http://") || profile_or_url.starts_with("https://");
    if is_url {
        let profile = ctx.profile.clone();
        config.default_profile = Some(profile.clone());
        config.profiles.entry(profile.clone()).or_default().project = Some(profile_or_url.clone());
        save_config(&ctx.config_path, &config)?;
        Ok(json!({"profile": profile, "project": profile_or_url, "saved": true}))
    } else {
        config.default_profile = Some(profile_or_url.clone());
        config.profiles.entry(profile_or_url.clone()).or_default();
        save_config(&ctx.config_path, &config)?;
        Ok(json!({"profile": profile_or_url, "saved": true}))
    }
}

fn handle_api_command(ctx: &Context, command: Commands) -> Result<Value, CliError> {
    if is_mutating(&command) && requires_confirmation(&command) && !ctx.yes && !ctx.dry_run {
        return Err(CliError::ApprovalRequired {
            command: command_name(&command),
            hint: "Rerun with --dry-run to preview or --yes to apply.".to_string(),
        });
    }

    let plan = plan_for(ctx, &command);
    if ctx.dry_run || (is_mutating(&command) && !ctx.yes) {
        append_audit(ctx, &command, &plan, "planned")?;
        return Ok(plan);
    }

    let request = bridge_request(ctx, &command)?;
    let result = invoke_bridge(ctx, &request)?;
    append_audit(ctx, &command, &result, "ok")?;
    Ok(result)
}

fn plan_for(ctx: &Context, command: &Commands) -> Value {
    json!({
        "command": command_name(command),
        "sdk_methods": sdk_methods_for(command),
        "mutating": is_mutating(command),
        "dry_run": true,
        "bridge": bridge_status(ctx),
        "status": "planned",
        "request": bridge_request_preview(command),
        "note": "Dry-run plan. Rerun without --dry-run for reads or with --yes for writes to call the Node framer-api bridge."
    })
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
    #[error("Bridge command {command} failed: {message}")]
    BridgeFailed {
        command: String,
        code: String,
        message: String,
        hint: Option<String>,
        retryable: bool,
        details: Option<Value>,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
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
            Self::BridgeFailed { code, .. } => match code.as_str() {
                "E_USAGE" => 2,
                "E_AUTH_MISSING" | "E_AUTH_INVALID" | "E_PERM_DENIED" => 3,
                "E_NOT_FOUND" => 4,
                "E_APPROVAL_REQUIRED" | "E_CONFLICT" | "E_SLUG_COLLISION" => 5,
                "E_RATE_LIMITED" => 6,
                "E_COLD_START_TIMEOUT" => 7,
                "E_NETWORK" => 8,
                _ => 10,
            },
            Self::Json(_) => 1,
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
                details: None,
            },
            CliError::BridgeFailed {
                command,
                code,
                message,
                hint,
                retryable,
                details,
            } => {
                let code: &'static str = Box::leak(code.into_boxed_str());
                Self {
                    code,
                    message: format!("Bridge command '{command}' failed: {message}"),
                    hint,
                    retryable,
                    sdk_method: None,
                    details,
                }
            }
            CliError::Json(err) => Self {
                code: "E_JSON",
                message: err.to_string(),
                hint: Some(
                    "The bridge returned invalid JSON or an input file could not be parsed."
                        .to_string(),
                ),
                retryable: false,
                sdk_method: None,
                details: None,
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

fn bridge_status(ctx: &Context) -> Value {
    json!({
        "kind": "node-framer-api",
        "available": ctx.bridge.as_ref().is_some_and(|path| path.exists()),
        "script": ctx.bridge,
        "env": {
            "FRAMER_API_KEY": env::var("FRAMER_API_KEY").is_ok(),
            "key_source": ctx.key_source,
            "FRAMERLI_BRIDGE": env::var("FRAMERLI_BRIDGE").ok()
        }
    })
}

fn append_audit(
    ctx: &Context,
    command: &Commands,
    payload: &Value,
    result: &str,
) -> Result<(), CliError> {
    if ctx.no_audit || !is_mutating(command) {
        return Ok(());
    }
    if let Err(err) = fs::create_dir_all(ctx.state_dir()) {
        eprintln!("framerli: audit log skipped: {err}");
        return Ok(());
    }
    let path = ctx.state_dir().join("audit.ndjson");
    let line = json!({
        "ts": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        "profile": ctx.profile,
        "cmd": command_name(command),
        "result": result,
        "dry_run": ctx.dry_run,
        "payload": payload
    });
    use std::io::Write;
    let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) else {
        eprintln!(
            "framerli: audit log skipped: could not open {}",
            path.display()
        );
        return Ok(());
    };
    if let Err(err) = writeln!(
        file,
        "{}",
        serde_json::to_string(&line).expect("audit line")
    ) {
        eprintln!("framerli: audit log skipped: {err}");
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct BridgeRequest {
    version: u32,
    operation: String,
    profile: String,
    project: Option<String>,
    dry_run: bool,
    args: Value,
}

#[derive(Debug, Deserialize)]
struct BridgeResponse {
    ok: bool,
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    error: Option<BridgeError>,
}

#[derive(Debug, Deserialize)]
struct BridgeError {
    code: Option<String>,
    message: String,
    hint: Option<String>,
    #[serde(default)]
    retryable: bool,
    details: Option<Value>,
}

fn default_bridge_path() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest_dir.join("bridge").join("framerli-bridge.mjs");
    candidate.exists().then_some(candidate)
}

fn bridge_request(ctx: &Context, command: &Commands) -> Result<BridgeRequest, CliError> {
    let (operation, args) = bridge_operation(command)?;
    Ok(BridgeRequest {
        version: 1,
        operation,
        profile: ctx.profile.clone(),
        project: ctx.project.clone(),
        dry_run: ctx.dry_run,
        args,
    })
}

fn bridge_request_preview(command: &Commands) -> Value {
    match bridge_operation(command) {
        Ok((operation, args)) => json!({"operation": operation, "args": args}),
        Err(_) => json!({"operation": null, "args": {}}),
    }
}

fn invoke_bridge(ctx: &Context, request: &BridgeRequest) -> Result<Value, CliError> {
    let Some(script) = &ctx.bridge else {
        return Err(CliError::BridgeUnavailable {
            command: request.operation.clone(),
            hint: "No bridge script was found. Set --bridge or FRAMERLI_BRIDGE to bridge/framerli-bridge.mjs.".to_string(),
        });
    };
    if !script.exists() {
        return Err(CliError::BridgeUnavailable {
            command: request.operation.clone(),
            hint: format!("Bridge script does not exist: {}", script.display()),
        });
    }

    let node = env::var("FRAMERLI_NODE").unwrap_or_else(|_| "node".to_string());
    let mut command = Command::new(node);
    command
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if env::var("FRAMER_API_KEY").is_err() {
        if let Some(name) = ctx
            .key_source
            .as_deref()
            .and_then(|source| source.strip_prefix("env:"))
        {
            if let Ok(value) = env::var(name) {
                command.env("FRAMER_API_KEY", value);
            }
        }
    }
    let mut child = command.spawn()?;

    {
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            CliError::Io(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "bridge stdin unavailable",
            ))
        })?;
        serde_json::to_writer(stdin, request)?;
    }

    let output = child.wait_with_output()?;
    let parsed: BridgeResponse = serde_json::from_slice(&output.stdout)?;
    if parsed.ok {
        Ok(parsed.data.unwrap_or_else(|| json!({})))
    } else {
        let err = parsed.error.unwrap_or(BridgeError {
            code: Some("E_BRIDGE".to_string()),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            hint: None,
            retryable: false,
            details: None,
        });
        Err(CliError::BridgeFailed {
            command: request.operation.clone(),
            code: err.code.unwrap_or_else(|| "E_BRIDGE".to_string()),
            message: err.message,
            hint: err.hint,
            retryable: err.retryable,
            details: err.details,
        })
    }
}

fn bridge_operation(command: &Commands) -> Result<(String, Value), CliError> {
    let pair = match command {
        Commands::Project(ProjectCommand::Info) => ("project.info", json!({})),
        Commands::Project(ProjectCommand::Audit) => ("project.audit", json!({})),
        Commands::Status => ("status", json!({})),
        Commands::Contributors(args) => ("contributors", json!({"from": args.from, "to": args.to})),
        Commands::Publish(args) => (
            "publish",
            json!({"promote": args.promote, "requireApproval": args.require_approval}),
        ),
        Commands::Deploy(DeployCommand::Promote { deployment_id }) => {
            ("deploy", json!({"deploymentId": deployment_id}))
        }
        Commands::Deploy(DeployCommand::Rollback) => ("deploy.rollback", json!({})),
        Commands::Cms(CmsCommand::Collections(CmsCollectionsCommand::List)) => {
            ("cms.collections.list", json!({}))
        }
        Commands::Cms(CmsCommand::Collection(CmsCollectionCommand::Show { slug })) => {
            ("cms.collection.show", json!({"collection": slug}))
        }
        Commands::Cms(CmsCommand::Fields(CmsFieldsCommand::List { collection })) => {
            ("cms.fields.list", json!({"collection": collection}))
        }
        Commands::Cms(CmsCommand::Fields(CmsFieldsCommand::Add(args))) => (
            "cms.fields.add",
            json!({"collection": args.collection, "name": args.name, "type": format!("{:?}", args.r#type), "cases": args.cases}),
        ),
        Commands::Cms(CmsCommand::Fields(CmsFieldsCommand::Remove {
            collection,
            field_id,
        })) => (
            "cms.fields.remove",
            json!({"collection": collection, "fieldId": field_id}),
        ),
        Commands::Cms(CmsCommand::Fields(CmsFieldsCommand::Reorder(args))) => (
            "cms.fields.reorder",
            json!({"collection": args.collection, "order": args.order}),
        ),
        Commands::Cms(CmsCommand::Items(CmsItemsCommand::List(args))) => (
            "cms.items.list",
            json!({"collection": args.collection, "where": args.r#where, "limit": args.limit, "cursor": args.cursor}),
        ),
        Commands::Cms(CmsCommand::Items(CmsItemsCommand::Get {
            collection,
            id_or_slug,
        })) => (
            "cms.items.get",
            json!({"collection": collection, "idOrSlug": id_or_slug}),
        ),
        Commands::Cms(CmsCommand::Items(CmsItemsCommand::Add(args))) => (
            "cms.items.add",
            json!({"collection": args.collection, "file": args.file, "update": args.update, "ifNotExists": args.if_not_exists}),
        ),
        Commands::Cms(CmsCommand::Items(CmsItemsCommand::Remove(args))) => (
            "cms.items.remove",
            json!({"collection": args.collection, "ids": args.ids}),
        ),
        Commands::Cms(CmsCommand::Items(CmsItemsCommand::Reorder(args))) => (
            "cms.items.reorder",
            json!({"collection": args.collection, "order": args.order}),
        ),
        Commands::Cms(CmsCommand::Import(args)) => (
            "cms.import",
            json!({"collection": args.collection, "from": format!("{:?}", args.from), "map": args.map, "file": args.file}),
        ),
        Commands::Cms(CmsCommand::Export(args)) => (
            "cms.export",
            json!({"collection": args.collection, "format": format!("{:?}", args.format), "out": args.out}),
        ),
        Commands::Cms(CmsCommand::Schema(CmsSchemaCommand::Dump { collection })) => {
            ("cms.schema.dump", json!({"collection": collection}))
        }
        Commands::Cms(CmsCommand::Schema(CmsSchemaCommand::Apply(args))) => {
            ("cms.schema.apply", json!({"file": args.file}))
        }
        Commands::Cms(CmsCommand::Schema(CmsSchemaCommand::Diff(args))) => {
            ("cms.schema.diff", json!({"file": args.file}))
        }
        Commands::Cms(CmsCommand::Sync(args)) => (
            "cms.sync",
            json!({"config": args.config, "watch": args.watch}),
        ),
        Commands::Whoami => ("whoami", json!({})),
        Commands::Can { method } => ("can", json!({"method": method})),
        Commands::Introspect(args) => ("introspect", json!({"depth": format!("{:?}", args.depth)})),
        Commands::Exec(args) => ("exec", json!({"script": args.script, "args": args.args})),
        Commands::Plan(args) => ("site.plan", json!({"file": args.file})),
        Commands::Apply(args) => (
            "site.apply",
            json!({"file": args.file, "autoApprove": args.auto_approve}),
        ),
        Commands::Diff(args) => ("site.diff", json!({"file": args.file})),
        other => {
            return Ok((command_name(other).replace(' ', "."), json!({})));
        }
    };
    Ok((pair.0.to_string(), pair.1))
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
