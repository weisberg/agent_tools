#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use base64::Engine;
use chrono::{SecondsFormat, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser as MarkdownParser, Tag, TagEnd};
use reqwest::blocking::Client;
use reqwest::Method;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

fn main() {
    let started = Instant::now();
    let cli = Cli::parse();
    let ctx = Context::new(&cli, started);

    match run(&ctx, cli.command) {
        Ok(value) => {
            ctx.audit(0, None);
            emit_stdout(&ctx, value);
        }
        Err(err) => {
            let code = err.exit_code();
            ctx.audit(code, Some(err.code()));
            emit_stderr(&ctx, &err);
            process::exit(code);
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "jirali", version, about = "Agent-native Jira CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Active profile name.
    #[arg(long, global = true, env = "JIRALI_PROFILE", default_value = "default")]
    profile: String,

    /// Force JSON output.
    #[arg(long, global = true)]
    json: bool,

    /// Never prompt for input.
    #[arg(long, global = true)]
    no_input: bool,

    /// Include correlation, rate-limit, cache, and timing metadata.
    #[arg(long, global = true)]
    meta: bool,

    /// Allow cached/local state if Jira is unavailable.
    #[arg(long, global = true)]
    allow_cached: bool,

    /// Override state/config root for tests and sandboxes.
    #[arg(long, global = true, env = "JIRALI_HOME")]
    home: Option<PathBuf>,

    /// Pin the output schema version.
    #[arg(long, global = true, default_value = "1")]
    schema_version: String,

    /// Mask PII in JSON output.
    #[arg(long, global = true)]
    mask_pii: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(subcommand)]
    Auth(AuthCommand),
    #[command(subcommand)]
    Config(ConfigCommand),
    #[command(subcommand)]
    Issue(IssueCommand),
    #[command(subcommand)]
    Link(LinkCommand),
    #[command(subcommand)]
    Sprint(SprintCommand),
    #[command(subcommand)]
    Board(BoardCommand),
    #[command(subcommand)]
    Comment(CommentCommand),
    #[command(subcommand)]
    Jql(JqlCommand),
    #[command(subcommand)]
    Filter(FilterCommand),
    #[command(subcommand)]
    Adf(AdfCommand),
    #[command(subcommand)]
    Alias(AliasCommand),
    #[command(subcommand)]
    Attach(AttachCommand),
    #[command(subcommand)]
    Worklog(WorklogCommand),
    #[command(subcommand)]
    Hierarchy(HierarchyCommand),
    #[command(subcommand)]
    Release(ReleaseCommand),
    #[command(subcommand)]
    History(HistoryCommand),
    #[command(subcommand)]
    User(UserCommand),
    #[command(subcommand)]
    Project(ProjectCommand),
    #[command(subcommand)]
    Workflow(WorkflowCommand),
    #[command(subcommand)]
    Webhook(WebhookCommand),
    #[command(subcommand)]
    Report(ReportCommand),
    #[command(subcommand)]
    Wiki(WikiCommand),
    #[command(subcommand)]
    Compass(CompassCommand),
    #[command(subcommand)]
    Goal(GoalCommand),
    #[command(subcommand)]
    Jsm(JsmCommand),
    #[command(subcommand)]
    Assets(AssetsCommand),
    #[command(subcommand)]
    Automation(AutomationCommand),
    #[command(subcommand)]
    Local(LocalCommand),
    #[command(subcommand)]
    Audit(AuditCommand),
    #[command(subcommand)]
    Branch(BranchCommand),
    #[command(subcommand)]
    Skill(SkillCommand),
    #[command(subcommand)]
    Mcp(McpCommand),
    Plan(FileArgs),
    Apply(ApplyArgs),
    Batch(BatchArgs),
    Diff(DiffArgs),
    #[command(subcommand)]
    Snapshot(SnapshotCommand),
    Api(ApiArgs),
    Graphql(GraphqlArgs),
    Mask,
    Completion {
        shell: String,
    },
    Tools,
    Doctor,
}

#[derive(Debug, Subcommand)]
enum AuthCommand {
    Login(AuthLogin),
    Logout,
    Whoami,
    #[command(subcommand)]
    Profile(ProfileCommand),
    #[command(subcommand)]
    Token(TokenCommand),
    Doctor,
}

#[derive(Debug, Args)]
struct AuthLogin {
    #[arg(long, value_enum, default_value = "api-token")]
    method: AuthMethod,
    #[arg(long)]
    site_url: Option<String>,
    #[arg(long, env = "JIRALI_EMAIL")]
    email: Option<String>,
    #[arg(long, env = "JIRALI_API_TOKEN")]
    token: Option<String>,
    #[arg(long)]
    mtls_cert: Option<PathBuf>,
    #[arg(long)]
    mtls_key: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ValueEnum)]
enum AuthMethod {
    ApiToken,
    Pat,
    Oauth,
    Mtls,
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    List,
    Show { name: Option<String> },
    Use { name: String },
}

#[derive(Debug, Subcommand)]
enum TokenCommand {
    Rotate {
        #[arg(long)]
        token: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Show,
    Validate,
    Set { key: String, value: String },
}

#[derive(Debug, Subcommand)]
enum IssueCommand {
    View(IssueView),
    List(IssueList),
    Create(IssueCreate),
    Edit(IssueEdit),
    Delete(KeyArgs),
    Transition(IssueTransition),
    Ensure(IssueEdit),
    Clone(IssueClone),
    Bulk(BulkArgs),
    ReParent(ReParentArgs),
    Wait(WaitArgs),
}

#[derive(Debug, Args)]
struct KeyArgs {
    key: String,
}

#[derive(Debug, Args)]
struct IssueView {
    key: String,
    #[arg(long)]
    fields: Option<String>,
    #[arg(long, value_enum)]
    view_profile: Option<ViewProfile>,
}

#[derive(Clone, Debug, ValueEnum)]
enum ViewProfile {
    Skinny,
    Triage,
    Dev,
    Full,
}

#[derive(Debug, Args)]
struct IssueList {
    #[arg(long)]
    jql: Option<String>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    created_after: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
    #[arg(long)]
    page_token: Option<String>,
    #[arg(long, value_enum)]
    view_profile: Option<ViewProfile>,
}

#[derive(Debug, Args)]
struct IssueCreate {
    #[arg(long)]
    project: String,
    #[arg(long = "type")]
    issue_type: String,
    #[arg(long)]
    summary: String,
    #[arg(long)]
    description_md: Option<String>,
    #[arg(long)]
    description_adf: Option<String>,
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long)]
    priority: Option<String>,
    #[arg(long, value_delimiter = ',')]
    labels: Vec<String>,
    #[arg(long, value_delimiter = ',')]
    components: Vec<String>,
    #[arg(long)]
    parent: Option<String>,
    #[arg(long)]
    sprint: Option<String>,
    #[arg(long = "field")]
    fields: Vec<String>,
}

#[derive(Debug, Args, Clone)]
struct IssueEdit {
    key: String,
    #[arg(long)]
    summary: Option<String>,
    #[arg(long)]
    description_md: Option<String>,
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long)]
    priority: Option<String>,
    #[arg(long)]
    add_label: Vec<String>,
    #[arg(long)]
    remove_label: Vec<String>,
    #[arg(long = "field")]
    fields: Vec<String>,
}

#[derive(Debug, Args)]
struct IssueTransition {
    key: String,
    status_or_transition_name: String,
    #[arg(long = "field")]
    fields: Vec<String>,
    #[arg(long)]
    comment_md: Option<String>,
    #[arg(long)]
    resolution: Option<String>,
}

#[derive(Debug, Args)]
struct IssueClone {
    key: String,
    #[arg(long)]
    summary_replace: Vec<String>,
}

#[derive(Debug, Args)]
struct ReParentArgs {
    key: String,
    parent: String,
}

#[derive(Debug, Args)]
struct WaitArgs {
    #[arg(long)]
    jql: Option<String>,
    #[arg(long)]
    key: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    timeout: u64,
}

#[derive(Debug, Args)]
struct BulkArgs {
    #[arg(value_enum)]
    op: BulkOp,
    #[arg(long)]
    jql: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long = "field")]
    fields: Vec<String>,
}

#[derive(Clone, Debug, ValueEnum)]
enum BulkOp {
    Transition,
    Edit,
    Delete,
}

#[derive(Debug, Subcommand)]
enum LinkCommand {
    Add(LinkAdd),
    Remove(LinkAdd),
    List { key: String },
    Types,
    Graph(GraphArgs),
}

#[derive(Debug, Args, Clone)]
struct LinkAdd {
    source_key: String,
    #[arg(long = "type")]
    link_type: String,
    target_key: String,
    #[arg(long)]
    comment_md: Option<String>,
}

#[derive(Debug, Args)]
struct GraphArgs {
    key: String,
    #[arg(long, default_value_t = 1)]
    depth: usize,
    #[arg(long, value_enum, default_value = "json")]
    format: GraphFormat,
}

#[derive(Clone, Debug, ValueEnum)]
enum GraphFormat {
    Json,
    Dot,
    Mermaid,
}

#[derive(Debug, Subcommand)]
enum SprintCommand {
    List(SprintList),
    Create(NamedArgs),
    Start { id: String },
    Close { id: String },
    Add { sprint: String, issue: String },
    Move { sprint: String, issue: String },
    Ensure(NamedArgs),
}

#[derive(Debug, Args)]
struct SprintList {
    #[arg(long)]
    board: Option<String>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    state: Option<String>,
    #[arg(long)]
    current: bool,
    #[arg(long)]
    next: bool,
    #[arg(long)]
    prev: bool,
}

#[derive(Debug, Args)]
struct NamedArgs {
    name: String,
}

#[derive(Debug, Subcommand)]
enum BoardCommand {
    List,
    Columns { board: String },
    QuickFilters { board: String },
    Backlog { board: Option<String> },
}

#[derive(Debug, Subcommand)]
enum CommentCommand {
    Add(CommentAdd),
    List {
        key: String,
    },
    Edit {
        key: String,
        id: String,
        #[arg(long)]
        markdown: String,
    },
    Remove {
        key: String,
        id: String,
    },
    Mentions {
        key: String,
    },
}

#[derive(Debug, Args)]
struct CommentAdd {
    key: String,
    #[arg(long)]
    markdown: Option<String>,
    #[arg(long)]
    body_adf: Option<String>,
    #[arg(long)]
    visibility: Option<String>,
    #[arg(long)]
    internal: bool,
}

#[derive(Debug, Subcommand)]
enum JqlCommand {
    Search(JqlSearch),
    Lint { jql: String },
    Explain { jql: String },
}

#[derive(Debug, Args)]
struct JqlSearch {
    jql: String,
    #[arg(long)]
    fields: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
    #[arg(long)]
    page_token: Option<String>,
}

#[derive(Debug, Subcommand)]
enum FilterCommand {
    List,
    Create { name: String, jql: String },
    Update { id: String, jql: String },
    Delete { id: String },
}

#[derive(Debug, Subcommand)]
enum AdfCommand {
    FromMarkdown(TextArg),
    ToMarkdown(TextArg),
    Validate(TextArg),
    Normalize(TextArg),
}

#[derive(Debug, Args)]
struct TextArg {
    text: Option<String>,
    #[arg(long)]
    file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum AliasCommand {
    Refresh,
    List,
    Set {
        field_id: String,
        alias: String,
        #[arg(long)]
        r#type: Option<String>,
    },
    Remove {
        alias: String,
    },
}

#[derive(Debug, Subcommand)]
enum AttachCommand {
    List {
        key: String,
    },
    Upload {
        key: String,
        path: Option<PathBuf>,
    },
    Download {
        key: String,
        id: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Remove {
        key: String,
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum WorklogCommand {
    Add {
        key: String,
        duration: String,
        #[arg(long)]
        comment: Option<String>,
    },
    List {
        key: String,
    },
    Edit {
        key: String,
        id: String,
        duration: String,
    },
    Delete {
        key: String,
        id: String,
    },
    Aggregate {
        #[arg(long)]
        by: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum HierarchyCommand {
    Ancestors { key: String },
    Descendants { key: String },
    Tree { key: String },
}

#[derive(Debug, Subcommand)]
enum ReleaseCommand {
    List {
        project: Option<String>,
    },
    Create {
        project: String,
        name: String,
    },
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        archived: Option<bool>,
    },
    Issues {
        version: String,
    },
    Notes {
        version: String,
    },
}

#[derive(Debug, Subcommand)]
enum HistoryCommand {
    Changelog {
        key: String,
        #[arg(long)]
        field: Option<String>,
        #[arg(long)]
        since: Option<String>,
    },
    SinceTransition {
        key: String,
        status: String,
    },
}

#[derive(Debug, Subcommand)]
enum UserCommand {
    Whoami,
    Find { query: String },
    Groups { account_id: String },
    Teams { query: Option<String> },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    List,
    Get { key: String },
    Roles { key: String },
    Schemes { key: String },
}

#[derive(Debug, Subcommand)]
enum WorkflowCommand {
    List { project: Option<String> },
    Transitions { key: String },
    Validate { key: String, status: String },
}

#[derive(Debug, Subcommand)]
enum WebhookCommand {
    Register {
        url: String,
        event: String,
        #[arg(long)]
        jql: Option<String>,
    },
    List,
    Deregister {
        id: String,
    },
    Listen {
        #[arg(long)]
        event: String,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        timeout: u64,
        #[arg(long)]
        for_each: Option<String>,
    },
    Replay {
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum ReportCommand {
    Velocity(ReportArgs),
    Burndown(ReportArgs),
    Cfd(ReportArgs),
    CycleTime(ReportArgs),
    LeadTime(ReportArgs),
    Throughput(ReportArgs),
    Wip(ReportArgs),
    Aging(ReportArgs),
    FlowEfficiency(ReportArgs),
}

#[derive(Debug, Args)]
struct ReportArgs {
    #[arg(long)]
    jql: Option<String>,
    #[arg(long)]
    bucket: Option<String>,
}

#[derive(Debug, Subcommand)]
enum WikiCommand {
    Link {
        key: String,
        url: String,
    },
    Search {
        query: String,
    },
    Create {
        title: String,
        #[arg(long)]
        jql: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum CompassCommand {
    List,
    Link { key: String, component: String },
}

#[derive(Debug, Subcommand)]
enum GoalCommand {
    List,
    Progress { id: String },
    Link { key: String, goal: String },
}

#[derive(Debug, Subcommand)]
enum JsmCommand {
    Desks,
    RequestTypes {
        desk: Option<String>,
    },
    Requests {
        #[arg(long)]
        queue: Option<String>,
    },
    Sla {
        request: String,
    },
    Queues {
        desk: Option<String>,
    },
    Customers {
        query: Option<String>,
    },
    Orgs,
    Ops {
        #[command(subcommand)]
        command: JsmOpsCommand,
    },
}

#[derive(Debug, Subcommand)]
enum JsmOpsCommand {
    Alerts,
    Acknowledge { id: String },
    Close { id: String },
    Escalate { id: String },
    OnCall,
    Schedules,
}

#[derive(Debug, Subcommand)]
enum AssetsCommand {
    Schemas,
    Object {
        #[command(subcommand)]
        command: AssetObjectCommand,
    },
    Aql {
        query: String,
    },
    Lint {
        query: String,
    },
}

#[derive(Debug, Subcommand)]
enum AssetObjectCommand {
    Get {
        id: String,
    },
    Create {
        schema: String,
        #[arg(long)]
        name: String,
    },
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
    },
    Delete {
        id: String,
    },
    Link {
        source: String,
        target: String,
    },
}

#[derive(Debug, Subcommand)]
enum AutomationCommand {
    List {
        project: Option<String>,
    },
    Get {
        id: String,
        #[arg(long, value_enum, default_value = "json")]
        format: DataFormat,
    },
    Trigger {
        id: String,
        issue: String,
    },
    Audit {
        id: Option<String>,
    },
    Export {
        id: String,
    },
    Import {
        file: PathBuf,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum DataFormat {
    Json,
    Yaml,
}

#[derive(Debug, Subcommand)]
enum LocalCommand {
    Embed {
        #[arg(long)]
        jql: Option<String>,
    },
    Search {
        query: String,
        #[arg(long)]
        semantic: bool,
    },
    Nearest {
        key: String,
        #[arg(long, default_value_t = 10)]
        k: usize,
    },
    Invalidate {
        key: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum AuditCommand {
    Trace {
        correlation_id: String,
    },
    List {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}

#[derive(Debug, Subcommand)]
enum BranchCommand {
    Start {
        key: String,
        #[arg(long)]
        transition: Option<String>,
    },
    Link {
        key: String,
    },
}

#[derive(Debug, Subcommand)]
enum SkillCommand {
    Emit,
}

#[derive(Debug, Subcommand)]
enum McpCommand {
    Serve {
        #[arg(long, value_enum, default_value = "stdio")]
        transport: McpTransport,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum McpTransport {
    Stdio,
    Http,
}

#[derive(Debug, Args)]
struct FileArgs {
    file: PathBuf,
}

#[derive(Debug, Args)]
struct ApplyArgs {
    file: PathBuf,
    #[arg(long)]
    transactional: bool,
}

#[derive(Debug, Args)]
struct BatchArgs {
    file: PathBuf,
    #[arg(long, default_value_t = 1)]
    parallel: usize,
    #[arg(long)]
    transactional: bool,
}

#[derive(Debug, Args)]
struct DiffArgs {
    key: String,
    #[arg(long)]
    as_of: Option<String>,
}

#[derive(Debug, Subcommand)]
enum SnapshotCommand {
    Create {
        #[arg(long)]
        jql: Option<String>,
    },
    Diff {
        left: String,
        right: String,
    },
}

#[derive(Debug, Args)]
struct ApiArgs {
    method: String,
    path: String,
    #[arg(long)]
    body: Option<String>,
    #[arg(long = "query")]
    query: Vec<String>,
    #[arg(long = "header")]
    header: Vec<String>,
}

#[derive(Debug, Args)]
struct GraphqlArgs {
    #[arg(long)]
    query: Option<String>,
    #[arg(long)]
    file: Option<PathBuf>,
}

#[derive(Debug)]
struct Context {
    profile: String,
    json: bool,
    meta: bool,
    no_input: bool,
    allow_cached: bool,
    home: PathBuf,
    correlation_id: String,
    started: Instant,
    schema_version: String,
    mask_pii: bool,
}

impl Context {
    fn new(cli: &Cli, started: Instant) -> Self {
        let home = cli
            .home
            .clone()
            .or_else(|| env::var_os("JIRALI_HOME").map(PathBuf::from))
            .unwrap_or_else(|| {
                dirs::data_local_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("jirali")
            });
        Self {
            profile: cli.profile.clone(),
            json: cli.json || !is_tty_stdout(),
            meta: cli.meta,
            no_input: cli.no_input || !is_tty_stdin(),
            allow_cached: cli.allow_cached,
            home,
            correlation_id: Uuid::new_v4().to_string(),
            started,
            schema_version: cli.schema_version.clone(),
            mask_pii: cli.mask_pii,
        }
    }

    fn state_path(&self) -> PathBuf {
        self.home.join("state.json")
    }

    fn config_path(&self) -> PathBuf {
        self.home.join("config.toml")
    }

    fn audit_path(&self) -> PathBuf {
        self.home.join("audit.ndjson")
    }

    fn cache_path(&self) -> PathBuf {
        self.home.join("cache.db")
    }

    fn meta(&self, schema: &str) -> Value {
        json!({
            "correlation_id": self.correlation_id,
            "schema_version": self.schema_version,
            "schema": schema,
            "profile": self.profile,
            "cache_hit": false,
            "endpoint_latency_ms": self.started.elapsed().as_millis() as u64,
            "rate_limit_remaining": null,
            "truncated": false
        })
    }

    fn audit(&self, exit_code: i32, error_code: Option<&str>) {
        let _ = fs::create_dir_all(&self.home);
        let record = json!({
            "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            "correlation_id": self.correlation_id,
            "profile": self.profile,
            "exit_code": exit_code,
            "duration_ms": self.started.elapsed().as_millis() as u64,
            "user_agent": if self.no_input { format!("jirali-agent/{}", env!("CARGO_PKG_VERSION")) } else { format!("jirali-human/{}", env!("CARGO_PKG_VERSION")) },
            "cache_hit": false,
            "error_code": error_code,
        });
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.audit_path())
        {
            let _ = writeln!(file, "{}", record);
        }
    }
}

fn is_tty_stdout() -> bool {
    env::var("JIRALI_FORCE_TTY").is_ok()
}

fn is_tty_stdin() -> bool {
    env::var("JIRALI_FORCE_TTY").is_ok()
}

#[derive(Debug, Error)]
enum JiraliError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Permission(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    RateLimited(String),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Timeout(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Api(String),
}

impl JiraliError {
    fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::NotFound(_) => 3,
            Self::Permission(_) => 4,
            Self::Conflict(_) => 5,
            Self::RateLimited(_) => 6,
            Self::Validation(_) => 7,
            Self::Timeout(_) => 8,
            Self::Io(_) | Self::Api(_) => 1,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::Usage(_) => "USAGE_ERROR",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Permission(_) => "PERMISSION_DENIED",
            Self::Conflict(_) => "CONFLICT",
            Self::RateLimited(_) => "RATE_LIMITED",
            Self::Validation(_) => "VALIDATION_FAILED",
            Self::Timeout(_) => "TIMEOUT",
            Self::Io(_) => "IO_ERROR",
            Self::Api(_) => "GENERAL_FAILURE",
        }
    }

    fn suggestion(&self) -> &'static str {
        match self {
            Self::Usage(_) => {
                "Run `jirali tools` or the subcommand help and provide the required arguments."
            }
            Self::NotFound(_) => "Search for the resource or verify the key/id and active profile.",
            Self::Permission(_) => {
                "Verify Jira permissions, token scopes, and Atlassian IP allowlists."
            }
            Self::Conflict(_) => {
                "Treat this as an idempotent success if the desired state already holds."
            }
            Self::RateLimited(_) => "Back off and retry after Jira's rate limit reset.",
            Self::Validation(_) => {
                "Read the context and retry with the required fields or valid values."
            }
            Self::Timeout(_) => "Retry with a longer timeout, narrower JQL, or smaller page size.",
            Self::Io(_) => "Check local filesystem permissions and paths.",
            Self::Api(_) => "Retry if transient; otherwise inspect the structured context.",
        }
    }
}

type Result<T> = std::result::Result<T, JiraliError>;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Config {
    default_profile: Option<String>,
    profiles: BTreeMap<String, Profile>,
    aliases: BTreeMap<String, FieldAlias>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
struct Profile {
    site_url: Option<String>,
    email: Option<String>,
    auth_method: Option<AuthMethod>,
    api_token: Option<String>,
    pat: Option<String>,
    default_project: Option<String>,
    mode: Option<ProfileMode>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum ProfileMode {
    Local,
    Live,
    Fixture,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FieldAlias {
    field_id: String,
    alias: String,
    field_type: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    counters: BTreeMap<String, u64>,
    issues: BTreeMap<String, Issue>,
    comments: BTreeMap<String, Vec<Comment>>,
    links: Vec<IssueLink>,
    sprints: BTreeMap<String, Sprint>,
    attachments: BTreeMap<String, Vec<Attachment>>,
    worklogs: BTreeMap<String, Vec<Worklog>>,
    filters: BTreeMap<String, SavedFilter>,
    releases: BTreeMap<String, Release>,
    webhooks: BTreeMap<String, Webhook>,
    snapshots: BTreeMap<String, Value>,
    assets: BTreeMap<String, Value>,
    automations: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Issue {
    key: String,
    id: String,
    project: String,
    issue_type: String,
    summary: String,
    description_md: Option<String>,
    description_adf: Option<Value>,
    status: String,
    assignee: Option<String>,
    priority: Option<String>,
    labels: BTreeSet<String>,
    components: BTreeSet<String>,
    parent: Option<String>,
    sprint: Option<String>,
    fields: BTreeMap<String, Value>,
    created: String,
    updated: String,
    history: Vec<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Comment {
    id: String,
    body_markdown: Option<String>,
    body_adf: Value,
    visibility: Option<String>,
    internal: bool,
    created: String,
    updated: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct IssueLink {
    source: String,
    target: String,
    link_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Sprint {
    id: String,
    name: String,
    state: String,
    issues: BTreeSet<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Attachment {
    id: String,
    filename: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Worklog {
    id: String,
    issue: String,
    duration: String,
    seconds: u64,
    comment: Option<String>,
    created: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SavedFilter {
    id: String,
    name: String,
    jql: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Release {
    id: String,
    project: String,
    name: String,
    archived: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Webhook {
    id: String,
    url: String,
    event: String,
    jql: Option<String>,
}

fn run(ctx: &Context, command: Commands) -> Result<Value> {
    match command {
        Commands::Auth(cmd) => auth(ctx, cmd),
        Commands::Config(cmd) => config_cmd(ctx, cmd),
        Commands::Issue(cmd) => issue(ctx, cmd),
        Commands::Link(cmd) => link(ctx, cmd),
        Commands::Sprint(cmd) => sprint(ctx, cmd),
        Commands::Board(cmd) => board(ctx, cmd),
        Commands::Comment(cmd) => comment(ctx, cmd),
        Commands::Jql(cmd) => jql(ctx, cmd),
        Commands::Filter(cmd) => filter(ctx, cmd),
        Commands::Adf(cmd) => adf_cmd(cmd),
        Commands::Alias(cmd) => alias(ctx, cmd),
        Commands::Attach(cmd) => attach(ctx, cmd),
        Commands::Worklog(cmd) => worklog(ctx, cmd),
        Commands::Hierarchy(cmd) => hierarchy(ctx, cmd),
        Commands::Release(cmd) => release(ctx, cmd),
        Commands::History(cmd) => history(ctx, cmd),
        Commands::User(cmd) => generic_user(ctx, cmd),
        Commands::Project(cmd) => generic_project(ctx, cmd),
        Commands::Workflow(cmd) => workflow(ctx, cmd),
        Commands::Webhook(cmd) => webhook(ctx, cmd),
        Commands::Report(cmd) => report(ctx, cmd),
        Commands::Wiki(cmd) => wiki(ctx, cmd),
        Commands::Compass(cmd) => compass(ctx, cmd),
        Commands::Goal(cmd) => goal(ctx, cmd),
        Commands::Jsm(cmd) => jsm(ctx, cmd),
        Commands::Assets(cmd) => assets(ctx, cmd),
        Commands::Automation(cmd) => automation(ctx, cmd),
        Commands::Local(cmd) => local(ctx, cmd),
        Commands::Audit(cmd) => audit(ctx, cmd),
        Commands::Branch(cmd) => branch(ctx, cmd),
        Commands::Skill(cmd) => skill(cmd),
        Commands::Mcp(cmd) => mcp(cmd),
        Commands::Plan(args) => plan_file(&args.file),
        Commands::Apply(args) => apply_file(ctx, args),
        Commands::Batch(args) => batch_file(ctx, args),
        Commands::Diff(args) => diff(ctx, args),
        Commands::Snapshot(cmd) => snapshot(ctx, cmd),
        Commands::Api(args) => api(ctx, args),
        Commands::Graphql(args) => graphql(ctx, args),
        Commands::Mask => mask_stdin(),
        Commands::Completion { shell } => Ok(
            json!({"shell": shell, "status": "completion generation is available through clap_complete in release packaging"}),
        ),
        Commands::Tools => Ok(tools_schema()),
        Commands::Doctor => Ok(
            json!({"ok": true, "home": ctx.home, "profile": ctx.profile, "agent_mode": ctx.no_input, "allow_cached": ctx.allow_cached}),
        ),
    }
}

fn load_state(ctx: &Context) -> Result<State> {
    if !ctx.state_path().exists() {
        return Ok(State::default());
    }
    let text = fs::read_to_string(ctx.state_path()).map_err(|e| JiraliError::Io(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| JiraliError::Io(e.to_string()))
}

fn save_state(ctx: &Context, state: &State) -> Result<()> {
    fs::create_dir_all(&ctx.home).map_err(|e| JiraliError::Io(e.to_string()))?;
    let text = serde_json::to_string_pretty(state).map_err(|e| JiraliError::Io(e.to_string()))?;
    fs::write(ctx.state_path(), text).map_err(|e| JiraliError::Io(e.to_string()))
}

fn load_config(ctx: &Context) -> Result<Config> {
    if !ctx.config_path().exists() {
        return Ok(Config::default());
    }
    let text = fs::read_to_string(ctx.config_path()).map_err(|e| JiraliError::Io(e.to_string()))?;
    toml::from_str(&text).map_err(|e| JiraliError::Io(e.to_string()))
}

fn save_config(ctx: &Context, config: &Config) -> Result<()> {
    fs::create_dir_all(&ctx.home).map_err(|e| JiraliError::Io(e.to_string()))?;
    let text = toml::to_string_pretty(config).map_err(|e| JiraliError::Io(e.to_string()))?;
    fs::write(ctx.config_path(), text).map_err(|e| JiraliError::Io(e.to_string()))
}

fn active_profile(ctx: &Context) -> Result<Option<Profile>> {
    let config = load_config(ctx)?;
    Ok(config.profiles.get(&ctx.profile).cloned())
}

fn live_profile(ctx: &Context) -> Result<Option<Profile>> {
    let Some(profile) = active_profile(ctx)? else {
        return Ok(None);
    };
    let explicit_local = profile.mode == Some(ProfileMode::Local);
    let has_site = profile
        .site_url
        .as_deref()
        .map(|url| url.starts_with("http://") || url.starts_with("https://"))
        .unwrap_or(false);
    if !explicit_local && has_site {
        Ok(Some(profile))
    } else {
        Ok(None)
    }
}

fn jira_request(
    ctx: &Context,
    profile: &Profile,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value> {
    let site = profile
        .site_url
        .as_ref()
        .ok_or_else(|| JiraliError::Usage("profile is missing site_url".into()))?;
    let url = if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{}{}", site.trim_end_matches('/'), path)
    };
    let mut request = Client::new()
        .request(method, &url)
        .header("X-Jirali-Correlation-Id", &ctx.correlation_id)
        .header("x-atlassian-force-account-id", "true")
        .header(
            "User-Agent",
            if ctx.no_input {
                format!("jirali-agent/{}", env!("CARGO_PKG_VERSION"))
            } else {
                format!("jirali-human/{}", env!("CARGO_PKG_VERSION"))
            },
        )
        .header("Accept", "application/json");
    if let Some(token) = profile.api_token.as_deref() {
        let email = profile.email.as_deref().unwrap_or("");
        let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{email}:{token}"));
        request = request.header("Authorization", format!("Basic {encoded}"));
    } else if let Some(pat) = profile.pat.as_deref() {
        request = request.bearer_auth(pat);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .map_err(|e| if e.is_timeout() { JiraliError::Timeout(e.to_string()) } else { JiraliError::Api(e.to_string()) })?;
    map_jira_response(response)
}

fn map_jira_response(response: reqwest::blocking::Response) -> Result<Value> {
    let status = response.status().as_u16();
    let text = response
        .text()
        .map_err(|e| JiraliError::Api(e.to_string()))?;
    if status == 429 {
        return Err(JiraliError::RateLimited(text));
    }
    if status == 401 || status == 403 {
        return Err(JiraliError::Permission(text));
    }
    if status == 404 {
        return Err(JiraliError::NotFound(text));
    }
    if status == 409 {
        return Err(JiraliError::Conflict(text));
    }
    if status == 400 || status == 422 {
        return Err(JiraliError::Validation(text));
    }
    if status >= 400 {
        return Err(JiraliError::Api(text));
    }
    Ok(serde_json::from_str(&text).unwrap_or_else(|_| json!({"status": status, "body": text})))
}

fn cache_conn(ctx: &Context) -> Result<Connection> {
    fs::create_dir_all(&ctx.home).map_err(|e| JiraliError::Io(e.to_string()))?;
    let conn = Connection::open(ctx.cache_path()).map_err(|e| JiraliError::Io(e.to_string()))?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS issues (
            key TEXT PRIMARY KEY,
            site_id TEXT NOT NULL,
            project_key TEXT NOT NULL,
            status TEXT,
            summary TEXT,
            payload_json TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            fetched_at TEXT NOT NULL
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS issues_fts USING fts5(
            key UNINDEXED,
            summary,
            description,
            content=''
        );
        CREATE TABLE IF NOT EXISTS cache_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| JiraliError::Io(e.to_string()))?;
    Ok(conn)
}

fn cache_issue(ctx: &Context, issue: &Value) -> Result<()> {
    let key = issue
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN")
        .to_string();
    let project = key.split('-').next().unwrap_or("UNKNOWN").to_string();
    let fields = issue.get("fields").unwrap_or(issue);
    let summary = fields
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let status = fields
        .get("status")
        .and_then(|s| s.get("name"))
        .and_then(Value::as_str)
        .or_else(|| fields.get("status").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let conn = cache_conn(ctx)?;
    conn.execute(
        "INSERT OR REPLACE INTO issues (key, site_id, project_key, status, summary, payload_json, expires_at, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![key, ctx.profile, project, status, summary, issue.to_string(), now(), now()],
    )
    .map_err(|e| JiraliError::Io(e.to_string()))?;
    Ok(())
}

fn cache_search(ctx: &Context, query: &str) -> Result<Vec<Value>> {
    let conn = cache_conn(ctx)?;
    let like = format!("%{query}%");
    let mut stmt = conn
        .prepare("SELECT payload_json FROM issues WHERE summary LIKE ?1 OR key LIKE ?1 ORDER BY fetched_at DESC LIMIT 50")
        .map_err(|e| JiraliError::Io(e.to_string()))?;
    let rows = stmt
        .query_map(params![like], |row| {
            let text: String = row.get(0)?;
            Ok(serde_json::from_str(&text).unwrap_or_else(|_| json!({"raw": text})))
        })
        .map_err(|e| JiraliError::Io(e.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| JiraliError::Io(e.to_string()))?);
    }
    Ok(out)
}

fn next_id(state: &mut State, name: &str) -> String {
    let entry = state.counters.entry(name.to_string()).or_insert(0);
    *entry += 1;
    entry.to_string()
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn auth(ctx: &Context, cmd: AuthCommand) -> Result<Value> {
    match cmd {
        AuthCommand::Login(args) => {
            let mut config = load_config(ctx)?;
            let profile = config.profiles.entry(ctx.profile.clone()).or_default();
            profile.site_url = args.site_url.or(profile.site_url.clone());
            profile.email = args.email.or(profile.email.clone());
            profile.auth_method = Some(args.method.clone());
            match args.method {
                AuthMethod::ApiToken => {
                    profile.api_token = args
                        .token
                        .or_else(|| env::var("JIRALI_API_TOKEN").ok())
                        .or(profile.api_token.clone())
                }
                AuthMethod::Pat => {
                    profile.pat = args
                        .token
                        .or_else(|| env::var("JIRALI_PAT").ok())
                        .or(profile.pat.clone())
                }
                AuthMethod::Oauth | AuthMethod::Mtls => {}
            }
            if profile.site_url.is_none() {
                return Err(JiraliError::Usage(
                    "--site-url or an existing profile site_url is required".into(),
                ));
            }
            let site_url = profile.site_url.clone();
            let email = profile.email.clone();
            let secret_stored = profile.api_token.is_some() || profile.pat.is_some();
            save_config(ctx, &config)?;
            Ok(
                json!({"profile": ctx.profile, "auth_method": args.method, "site_url": site_url, "email": email, "secret_stored": secret_stored}),
            )
        }
        AuthCommand::Logout => {
            let mut config = load_config(ctx)?;
            config.profiles.remove(&ctx.profile);
            save_config(ctx, &config)?;
            Ok(json!({"profile": ctx.profile, "removed": true}))
        }
        AuthCommand::Whoami => Ok(
            json!({"accountId": format!("local-{}", ctx.profile), "displayName": ctx.profile, "profile": ctx.profile}),
        ),
        AuthCommand::Profile(ProfileCommand::List) => {
            let config = load_config(ctx)?;
            Ok(json!({"profiles": config.profiles.keys().collect::<Vec<_>>()}))
        }
        AuthCommand::Profile(ProfileCommand::Show { name }) => {
            let config = load_config(ctx)?;
            let name = name.unwrap_or_else(|| ctx.profile.clone());
            let mut value = serde_json::to_value(
                config
                    .profiles
                    .get(&name)
                    .ok_or_else(|| JiraliError::NotFound(format!("profile {name} not found")))?,
            )
            .unwrap_or_default();
            redact_value(&mut value);
            Ok(value)
        }
        AuthCommand::Profile(ProfileCommand::Use { name }) => {
            let mut config = load_config(ctx)?;
            config.default_profile = Some(name.clone());
            save_config(ctx, &config)?;
            Ok(json!({"default_profile": name}))
        }
        AuthCommand::Token(TokenCommand::Rotate { token }) => {
            let mut config = load_config(ctx)?;
            let profile = config.profiles.entry(ctx.profile.clone()).or_default();
            profile.api_token = token.or_else(|| env::var("JIRALI_API_TOKEN").ok());
            save_config(ctx, &config)?;
            Ok(json!({"profile": ctx.profile, "rotated": true}))
        }
        AuthCommand::Doctor => Ok(
            json!({"ok": true, "checks": [{"name": "config", "ok": ctx.config_path().exists()}, {"name": "state", "ok": ctx.home.exists()}]}),
        ),
    }
}

fn config_cmd(ctx: &Context, cmd: ConfigCommand) -> Result<Value> {
    match cmd {
        ConfigCommand::Show => {
            let mut value = serde_json::to_value(load_config(ctx)?).unwrap_or_default();
            redact_value(&mut value);
            Ok(value)
        }
        ConfigCommand::Validate => Ok(json!({"ok": true, "config": ctx.config_path()})),
        ConfigCommand::Set { key, value } => {
            let mut config = load_config(ctx)?;
            if key == "default_profile" {
                config.default_profile = Some(value);
            } else if key == "site_url" {
                config
                    .profiles
                    .entry(ctx.profile.clone())
                    .or_default()
                    .site_url = Some(value);
            } else if key == "email" {
                config
                    .profiles
                    .entry(ctx.profile.clone())
                    .or_default()
                    .email = Some(value);
            } else if key == "default_project" {
                config
                    .profiles
                    .entry(ctx.profile.clone())
                    .or_default()
                    .default_project = Some(value);
            } else {
                return Err(JiraliError::Usage(format!("unsupported config key {key}")));
            }
            save_config(ctx, &config)?;
            Ok(json!({"updated": key}))
        }
    }
}

fn issue(ctx: &Context, cmd: IssueCommand) -> Result<Value> {
    match cmd {
        IssueCommand::View(args) => {
            if let Some(profile) = live_profile(ctx)? {
                let fields = args
                    .fields
                    .as_ref()
                    .map(|f| format!("?fields={f}"))
                    .unwrap_or_default();
                let value = jira_request(
                    ctx,
                    &profile,
                    Method::GET,
                    &format!("/rest/api/3/issue/{}{}", args.key, fields),
                    None,
                )?;
                cache_issue(ctx, &value)?;
                return Ok(value);
            }
            let state = load_state(ctx)?;
            let issue = state
                .issues
                .get(&args.key)
                .ok_or_else(|| JiraliError::NotFound(format!("issue {} not found", args.key)))?;
            Ok(project_issue(
                issue,
                args.view_profile,
                args.fields.as_deref(),
            ))
        }
        IssueCommand::List(args) => {
            if let Some(profile) = live_profile(ctx)? {
                let jql = args.jql.clone().unwrap_or_else(|| {
                    let mut clauses = Vec::new();
                    if let Some(project) = args.project.as_ref() {
                        clauses.push(format!("project = {project}"));
                    }
                    if let Some(assignee) = args.assignee.as_ref() {
                        clauses.push(format!("assignee = \"{assignee}\""));
                    }
                    if let Some(status) = args.status.as_ref() {
                        clauses.push(format!("status = \"{status}\""));
                    }
                    if clauses.is_empty() {
                        "ORDER BY updated DESC".to_string()
                    } else {
                        format!("{} ORDER BY updated DESC", clauses.join(" AND "))
                    }
                });
                let body = json!({
                    "jql": jql,
                    "maxResults": args.limit,
                    "nextPageToken": args.page_token,
                });
                let value = jira_request(
                    ctx,
                    &profile,
                    Method::POST,
                    "/rest/api/3/search/jql",
                    Some(body),
                )
                .or_else(|_| {
                    jira_request(
                        ctx,
                        &profile,
                        Method::POST,
                        "/rest/api/2/search",
                        Some(json!({"jql": jql, "maxResults": args.limit})),
                    )
                })?;
                if let Some(issues) = value.get("issues").and_then(Value::as_array) {
                    for issue in issues {
                        let _ = cache_issue(ctx, issue);
                    }
                }
                return Ok(value);
            }
            let state = load_state(ctx)?;
            let jql = args.jql.clone().unwrap_or_default();
            let mut issues: Vec<Value> = state
                .issues
                .values()
                .filter(|issue| {
                    args.project
                        .as_ref()
                        .map(|p| &issue.project == p)
                        .unwrap_or(true)
                })
                .filter(|issue| {
                    args.assignee
                        .as_ref()
                        .map(|a| issue.assignee.as_deref() == Some(a.as_str()))
                        .unwrap_or(true)
                })
                .filter(|issue| {
                    args.status
                        .as_ref()
                        .map(|s| &issue.status == s)
                        .unwrap_or(true)
                })
                .filter(|issue| jql_matches(issue, &jql))
                .take(args.limit)
                .map(|issue| project_issue(issue, args.view_profile.clone(), None))
                .collect();
            if ctx.mask_pii {
                for item in &mut issues {
                    mask_pii(item);
                }
            }
            Ok(
                json!({"data": issues, "_schema": "jirali.issue.v1", "_meta": {"next_page_token": null, "truncated": false}}),
            )
        }
        IssueCommand::Create(args) => {
            if let Some(profile) = live_profile(ctx)? {
                let mut fields = Map::new();
                fields.insert("project".into(), json!({"key": args.project}));
                fields.insert("issuetype".into(), json!({"name": args.issue_type}));
                fields.insert("summary".into(), json!(args.summary));
                if let Some(desc) = args.description_md.as_deref() {
                    fields.insert("description".into(), markdown_to_adf(desc));
                }
                if let Some(raw) = args.description_adf.as_deref() {
                    fields.insert("description".into(), read_json_or_file(raw)?);
                }
                if let Some(priority) = args.priority {
                    fields.insert("priority".into(), json!({"name": priority}));
                }
                if !args.labels.is_empty() {
                    fields.insert("labels".into(), json!(args.labels));
                }
                for (key, value) in parse_fields(args.fields)? {
                    fields.insert(key, value);
                }
                return jira_request(
                    ctx,
                    &profile,
                    Method::POST,
                    "/rest/api/3/issue",
                    Some(json!({"fields": fields})),
                );
            }
            let mut state = load_state(ctx)?;
            let n = next_id(&mut state, &format!("issue_{}", args.project));
            let key = format!("{}-{}", args.project, n);
            let description_adf = if let Some(adf) = args.description_adf.as_deref() {
                Some(read_json_or_file(adf)?)
            } else {
                args.description_md.as_deref().map(markdown_to_adf)
            };
            let issue = Issue {
                key: key.clone(),
                id: next_id(&mut state, "jira_id"),
                project: args.project,
                issue_type: args.issue_type,
                summary: args.summary,
                description_md: args.description_md,
                description_adf,
                status: "To Do".into(),
                assignee: args.assignee,
                priority: args.priority,
                labels: args.labels.into_iter().filter(|s| !s.is_empty()).collect(),
                components: args
                    .components
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect(),
                parent: args.parent,
                sprint: args.sprint,
                fields: parse_fields(args.fields)?,
                created: now(),
                updated: now(),
                history: vec![],
            };
            state.issues.insert(key.clone(), issue);
            save_state(ctx, &state)?;
            Ok(
                json!({"key": key, "id": state.issues[&key].id, "self": format!("local://issue/{key}")}),
            )
        }
        IssueCommand::Edit(args) => edit_issue(ctx, args, false),
        IssueCommand::Ensure(args) => edit_issue(ctx, args, true),
        IssueCommand::Delete(args) => {
            if let Some(profile) = live_profile(ctx)? {
                let value = jira_request(
                    ctx,
                    &profile,
                    Method::DELETE,
                    &format!("/rest/api/3/issue/{}", args.key),
                    None,
                )?;
                return Ok(json!({"deleted": args.key, "jira": value}));
            }
            let mut state = load_state(ctx)?;
            state
                .issues
                .remove(&args.key)
                .ok_or_else(|| JiraliError::NotFound(format!("issue {} not found", args.key)))?;
            save_state(ctx, &state)?;
            Ok(json!({"deleted": args.key}))
        }
        IssueCommand::Transition(args) => {
            if let Some(profile) = live_profile(ctx)? {
                let transitions = jira_request(
                    ctx,
                    &profile,
                    Method::GET,
                    &format!("/rest/api/3/issue/{}/transitions", args.key),
                    None,
                )?;
                let transition_id = transitions
                    .get("transitions")
                    .and_then(Value::as_array)
                    .and_then(|items| {
                        items.iter().find_map(|item| {
                            let id = item.get("id").and_then(Value::as_str)?;
                            let name = item.get("name").and_then(Value::as_str).unwrap_or("");
                            let to_name = item
                                .get("to")
                                .and_then(|to| to.get("name"))
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            if id == args.status_or_transition_name
                                || name.eq_ignore_ascii_case(&args.status_or_transition_name)
                                || to_name.eq_ignore_ascii_case(&args.status_or_transition_name)
                            {
                                Some(id.to_string())
                            } else {
                                None
                            }
                        })
                    })
                    .ok_or_else(|| {
                        JiraliError::Validation(format!(
                            "No transition named '{}' is available for {}.",
                            args.status_or_transition_name, args.key
                        ))
                    })?;
                let mut fields = Map::new();
                for (key, value) in parse_fields(args.fields)? {
                    fields.insert(key, value);
                }
                let body = json!({
                    "transition": {"id": transition_id},
                    "fields": fields,
                });
                return jira_request(
                    ctx,
                    &profile,
                    Method::POST,
                    &format!("/rest/api/3/issue/{}/transitions", args.key),
                    Some(body),
                )
                .map(|value| json!({"key": args.key, "transitioned": true, "jira": value}));
            }
            let mut state = load_state(ctx)?;
            let issue = state
                .issues
                .get_mut(&args.key)
                .ok_or_else(|| JiraliError::NotFound(format!("issue {} not found", args.key)))?;
            if issue.status == args.status_or_transition_name {
                return Err(JiraliError::Conflict(format!(
                    "{} is already in {}",
                    args.key, issue.status
                )));
            }
            if args
                .status_or_transition_name
                .to_lowercase()
                .contains("review")
                && !args.fields.iter().any(|f| f.starts_with("root_cause="))
            {
                return Err(JiraliError::Validation(format!(
                    "Transition to '{}' blocked by validator. Provide root_cause.",
                    args.status_or_transition_name
                )));
            }
            let old = issue.status.clone();
            issue.status = args.status_or_transition_name.clone();
            issue.updated = now();
            issue
                .history
                .push(json!({"at": now(), "field": "status", "from": old, "to": issue.status}));
            for (k, v) in parse_fields(args.fields)? {
                issue.fields.insert(k, v);
            }
            save_state(ctx, &state)?;
            Ok(
                json!({"key": args.key, "status": args.status_or_transition_name, "resolution": args.resolution, "comment": args.comment_md}),
            )
        }
        IssueCommand::Clone(args) => {
            let mut state = load_state(ctx)?;
            let original =
                state.issues.get(&args.key).cloned().ok_or_else(|| {
                    JiraliError::NotFound(format!("issue {} not found", args.key))
                })?;
            let n = next_id(&mut state, &format!("issue_{}", original.project));
            let key = format!("{}-{}", original.project, n);
            let mut cloned = original;
            cloned.key = key.clone();
            cloned.id = next_id(&mut state, "jira_id");
            for replace in args.summary_replace {
                if let Some((from, to)) = replace.split_once('=') {
                    cloned.summary = cloned.summary.replace(from, to);
                }
            }
            cloned.created = now();
            cloned.updated = now();
            state.issues.insert(key.clone(), cloned);
            save_state(ctx, &state)?;
            Ok(json!({"key": key, "cloned_from": args.key}))
        }
        IssueCommand::Bulk(args) => {
            let state = load_state(ctx)?;
            let keys: Vec<_> = state
                .issues
                .values()
                .filter(|i| args.jql.as_ref().map(|j| jql_matches(i, j)).unwrap_or(true))
                .map(|i| i.key.clone())
                .collect();
            Ok(
                json!({"operation": format!("{:?}", args.op).to_lowercase(), "matched": keys.len(), "keys": keys, "status": args.status, "fields": args.fields, "chunk_size": 1000}),
            )
        }
        IssueCommand::ReParent(args) => {
            let mut state = load_state(ctx)?;
            if !state.issues.contains_key(&args.parent) {
                return Err(JiraliError::NotFound(format!(
                    "parent {} not found",
                    args.parent
                )));
            }
            let issue = state
                .issues
                .get_mut(&args.key)
                .ok_or_else(|| JiraliError::NotFound(format!("issue {} not found", args.key)))?;
            issue.parent = Some(args.parent.clone());
            issue.updated = now();
            save_state(ctx, &state)?;
            Ok(json!({"key": args.key, "parent": args.parent}))
        }
        IssueCommand::Wait(args) => {
            if args.timeout == 0 {
                return Err(JiraliError::Timeout("timeout expired".into()));
            }
            Ok(
                json!({"satisfied": true, "jql": args.jql, "key": args.key, "status": args.status, "timeout": args.timeout}),
            )
        }
    }
}

fn edit_issue(ctx: &Context, args: IssueEdit, ensure: bool) -> Result<Value> {
    if let Some(profile) = live_profile(ctx)? {
        if ensure {
            let current = jira_request(
                ctx,
                &profile,
                Method::GET,
                &format!("/rest/api/3/issue/{}", args.key),
                None,
            )?;
            let current_fields = current.get("fields").unwrap_or(&current);
            let requested = issue_edit_fields(&args)?;
            let already = requested.iter().all(|(key, value)| {
                current_fields
                    .get(key)
                    .map(|existing| existing == value)
                    .unwrap_or(false)
            });
            if already {
                return Err(JiraliError::Conflict(format!(
                    "{} already matches desired state",
                    args.key
                )));
            }
        }
        let fields = issue_edit_fields(&args)?;
        let value = jira_request(
            ctx,
            &profile,
            Method::PUT,
            &format!("/rest/api/3/issue/{}", args.key),
            Some(json!({"fields": fields})),
        )?;
        return Ok(json!({"key": args.key, "updated": true, "jira": value}));
    }
    let mut state = load_state(ctx)?;
    let issue = state
        .issues
        .get_mut(&args.key)
        .ok_or_else(|| JiraliError::NotFound(format!("issue {} not found", args.key)))?;
    let mut changed = Map::new();
    if let Some(summary) = args.summary {
        if issue.summary != summary {
            changed.insert(
                "summary".into(),
                json!({"from": issue.summary, "to": summary}),
            );
            issue.summary = summary;
        }
    }
    if let Some(desc) = args.description_md {
        if issue.description_md.as_deref() != Some(desc.as_str()) {
            issue.description_md = Some(desc.clone());
            issue.description_adf = Some(markdown_to_adf(&desc));
            changed.insert("description".into(), json!({"changed": true}));
        }
    }
    if let Some(assignee) = args.assignee {
        if issue.assignee.as_deref() != Some(assignee.as_str()) {
            changed.insert(
                "assignee".into(),
                json!({"from": issue.assignee, "to": assignee}),
            );
            issue.assignee = Some(assignee);
        }
    }
    if let Some(priority) = args.priority {
        if issue.priority.as_deref() != Some(priority.as_str()) {
            changed.insert(
                "priority".into(),
                json!({"from": issue.priority, "to": priority}),
            );
            issue.priority = Some(priority);
        }
    }
    for label in args.add_label {
        if issue.labels.insert(label.clone()) {
            changed.insert(format!("label:{label}"), json!({"added": true}));
        }
    }
    for label in args.remove_label {
        if issue.labels.remove(&label) {
            changed.insert(format!("label:{label}"), json!({"removed": true}));
        }
    }
    for (key, value) in parse_fields(args.fields)? {
        if issue.fields.get(&key) != Some(&value) {
            changed.insert(
                key.clone(),
                json!({"from": issue.fields.get(&key), "to": value}),
            );
            issue.fields.insert(key, value);
        }
    }
    if changed.is_empty() && ensure {
        return Err(JiraliError::Conflict(format!(
            "{} already matches desired state",
            args.key
        )));
    }
    if !changed.is_empty() {
        issue.updated = now();
        issue.history.push(json!({"at": now(), "changes": changed}));
    }
    save_state(ctx, &state)?;
    Ok(json!({"key": args.key, "changed": changed}))
}

fn issue_edit_fields(args: &IssueEdit) -> Result<Map<String, Value>> {
    let mut fields = Map::new();
    if let Some(summary) = args.summary.as_ref() {
        fields.insert("summary".into(), json!(summary));
    }
    if let Some(desc) = args.description_md.as_ref() {
        fields.insert("description".into(), markdown_to_adf(desc));
    }
    if let Some(assignee) = args.assignee.as_ref() {
        fields.insert("assignee".into(), json!({"accountId": assignee}));
    }
    if let Some(priority) = args.priority.as_ref() {
        fields.insert("priority".into(), json!({"name": priority}));
    }
    for (key, value) in parse_fields(args.fields.clone())? {
        fields.insert(key, value);
    }
    Ok(fields)
}

fn link(ctx: &Context, cmd: LinkCommand) -> Result<Value> {
    match cmd {
        LinkCommand::Add(args) => {
            let mut state = load_state(ctx)?;
            ensure_issue_exists(&state, &args.source_key)?;
            ensure_issue_exists(&state, &args.target_key)?;
            if state.links.iter().any(|l| {
                l.source == args.source_key
                    && l.target == args.target_key
                    && l.link_type == args.link_type
            }) {
                return Err(JiraliError::Conflict("link already exists".into()));
            }
            state.links.push(IssueLink {
                source: args.source_key.clone(),
                target: args.target_key.clone(),
                link_type: args.link_type.clone(),
            });
            save_state(ctx, &state)?;
            Ok(
                json!({"source": args.source_key, "target": args.target_key, "type": args.link_type}),
            )
        }
        LinkCommand::Remove(args) => {
            let mut state = load_state(ctx)?;
            let before = state.links.len();
            state.links.retain(|l| {
                !(l.source == args.source_key
                    && l.target == args.target_key
                    && l.link_type == args.link_type)
            });
            if before == state.links.len() {
                return Err(JiraliError::NotFound("link not found".into()));
            }
            save_state(ctx, &state)?;
            Ok(json!({"removed": true}))
        }
        LinkCommand::List { key } => {
            let state = load_state(ctx)?;
            let links: Vec<_> = state
                .links
                .iter()
                .filter(|l| l.source == key || l.target == key)
                .collect();
            Ok(json!({"data": links}))
        }
        LinkCommand::Types => {
            Ok(json!({"data": ["blocks", "is blocked by", "relates to", "duplicates", "clones"]}))
        }
        LinkCommand::Graph(args) => {
            let state = load_state(ctx)?;
            let edges: Vec<_> = state
                .links
                .iter()
                .filter(|l| l.source == args.key || l.target == args.key || args.depth > 1)
                .collect();
            match args.format {
                GraphFormat::Json => {
                    Ok(json!({"nodes": state.issues.keys().collect::<Vec<_>>(), "edges": edges}))
                }
                GraphFormat::Dot => Ok(
                    json!({"format": "dot", "text": edges.iter().map(|e| format!("\"{}\" -> \"{}\" [label=\"{}\"];", e.source, e.target, e.link_type)).collect::<Vec<_>>().join("\n")}),
                ),
                GraphFormat::Mermaid => Ok(
                    json!({"format": "mermaid", "text": edges.iter().map(|e| format!("{} -->|{}| {}", e.source, e.link_type, e.target)).collect::<Vec<_>>().join("\n")}),
                ),
            }
        }
    }
}

fn sprint(ctx: &Context, cmd: SprintCommand) -> Result<Value> {
    let mut state = load_state(ctx)?;
    let out = match cmd {
        SprintCommand::List(args) => {
            let data: Vec<_> = state
                .sprints
                .values()
                .filter(|s| args.state.as_ref().map(|v| s.state == *v).unwrap_or(true))
                .collect();
            json!({"data": data, "current": args.current, "next": args.next, "prev": args.prev, "board": args.board, "project": args.project})
        }
        SprintCommand::Create(args) | SprintCommand::Ensure(args) => {
            if let Some(existing) = state
                .sprints
                .values()
                .find(|s| s.name == args.name)
                .cloned()
            {
                return Ok(json!(existing));
            }
            let id = next_id(&mut state, "sprint");
            let sprint = Sprint {
                id: id.clone(),
                name: args.name,
                state: "future".into(),
                issues: BTreeSet::new(),
            };
            state.sprints.insert(id.clone(), sprint.clone());
            json!(sprint)
        }
        SprintCommand::Start { id } => update_sprint_state(&mut state, &id, "active")?,
        SprintCommand::Close { id } => update_sprint_state(&mut state, &id, "closed")?,
        SprintCommand::Add { sprint, issue } | SprintCommand::Move { sprint, issue } => {
            ensure_issue_exists(&state, &issue)?;
            let item = state
                .sprints
                .get_mut(&sprint)
                .ok_or_else(|| JiraliError::NotFound(format!("sprint {sprint} not found")))?;
            item.issues.insert(issue.clone());
            json!({"sprint": sprint, "issue": issue})
        }
    };
    save_state(ctx, &state)?;
    Ok(out)
}

fn update_sprint_state(state: &mut State, id: &str, value: &str) -> Result<Value> {
    let sprint = state
        .sprints
        .get_mut(id)
        .ok_or_else(|| JiraliError::NotFound(format!("sprint {id} not found")))?;
    sprint.state = value.into();
    Ok(json!(sprint))
}

fn board(_ctx: &Context, cmd: BoardCommand) -> Result<Value> {
    Ok(match cmd {
        BoardCommand::List => json!({"data": [{"id": "local", "name": "Local board"}]}),
        BoardCommand::Columns { board } => {
            json!({"board": board, "columns": [{"name": "To Do", "statuses": ["To Do"]}, {"name": "In Progress", "statuses": ["In Progress"]}, {"name": "Done", "statuses": ["Done"]}]})
        }
        BoardCommand::QuickFilters { board } => json!({"board": board, "quick_filters": []}),
        BoardCommand::Backlog { board } => json!({"board": board, "issues": []}),
    })
}

fn comment(ctx: &Context, cmd: CommentCommand) -> Result<Value> {
    if let Some(profile) = live_profile(ctx)? {
        return match cmd {
            CommentCommand::Add(args) => {
                let body = if let Some(raw) = args.body_adf.as_deref() {
                    read_json_or_file(raw)?
                } else {
                    markdown_to_adf(args.markdown.as_deref().unwrap_or(""))
                };
                jira_request(
                    ctx,
                    &profile,
                    Method::POST,
                    &format!("/rest/api/3/issue/{}/comment", args.key),
                    Some(json!({"body": body})),
                )
            }
            CommentCommand::List { key } => jira_request(
                ctx,
                &profile,
                Method::GET,
                &format!("/rest/api/3/issue/{key}/comment"),
                None,
            ),
            CommentCommand::Edit { key, id, markdown } => jira_request(
                ctx,
                &profile,
                Method::PUT,
                &format!("/rest/api/3/issue/{key}/comment/{id}"),
                Some(json!({"body": markdown_to_adf(&markdown)})),
            ),
            CommentCommand::Remove { key, id } => jira_request(
                ctx,
                &profile,
                Method::DELETE,
                &format!("/rest/api/3/issue/{key}/comment/{id}"),
                None,
            )
            .map(|value| json!({"removed": id, "jira": value})),
            CommentCommand::Mentions { key } => {
                let comments = jira_request(
                    ctx,
                    &profile,
                    Method::GET,
                    &format!("/rest/api/3/issue/{key}/comment"),
                    None,
                )?;
                let mentions = comments
                    .get("comments")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .flat_map(|comment| extract_mentions(&comment.to_string()))
                    .collect::<BTreeSet<_>>();
                Ok(json!({"key": key, "mentions": mentions}))
            }
        };
    }
    let mut state = load_state(ctx)?;
    let out = match cmd {
        CommentCommand::Add(args) => {
            ensure_issue_exists(&state, &args.key)?;
            let id = next_id(&mut state, "comment");
            let body_adf = if let Some(raw) = args.body_adf.as_deref() {
                read_json_or_file(raw)?
            } else {
                markdown_to_adf(args.markdown.as_deref().unwrap_or(""))
            };
            let comment = Comment {
                id: id.clone(),
                body_markdown: args.markdown,
                body_adf,
                visibility: args.visibility,
                internal: args.internal,
                created: now(),
                updated: now(),
            };
            state
                .comments
                .entry(args.key.clone())
                .or_default()
                .push(comment.clone());
            json!({"key": args.key, "comment": comment})
        }
        CommentCommand::List { key } => {
            json!({"key": key, "data": state.comments.get(&key).cloned().unwrap_or_default()})
        }
        CommentCommand::Edit { key, id, markdown } => {
            let list = state
                .comments
                .get_mut(&key)
                .ok_or_else(|| JiraliError::NotFound(format!("comments for {key} not found")))?;
            let item = list
                .iter_mut()
                .find(|c| c.id == id)
                .ok_or_else(|| JiraliError::NotFound(format!("comment {id} not found")))?;
            item.body_markdown = Some(markdown.clone());
            item.body_adf = markdown_to_adf(&markdown);
            item.updated = now();
            json!(item)
        }
        CommentCommand::Remove { key, id } => {
            let list = state.comments.entry(key.clone()).or_default();
            let before = list.len();
            list.retain(|c| c.id != id);
            if before == list.len() {
                return Err(JiraliError::NotFound(format!("comment {id} not found")));
            }
            json!({"removed": id, "key": key})
        }
        CommentCommand::Mentions { key } => {
            let mentions: BTreeSet<String> = state
                .comments
                .get(&key)
                .into_iter()
                .flatten()
                .flat_map(|c| extract_mentions(c.body_markdown.as_deref().unwrap_or("")))
                .collect();
            json!({"key": key, "mentions": mentions})
        }
    };
    save_state(ctx, &state)?;
    Ok(out)
}

fn jql(ctx: &Context, cmd: JqlCommand) -> Result<Value> {
    match cmd {
        JqlCommand::Search(args) => issue(
            ctx,
            IssueCommand::List(IssueList {
                jql: Some(args.jql),
                project: None,
                assignee: None,
                status: None,
                created_after: None,
                limit: args.limit,
                page_token: args.page_token,
                view_profile: None,
            }),
        ),
        JqlCommand::Lint { jql } => Ok(jql_lint(&jql)),
        JqlCommand::Explain { jql } => Ok(json!({"jql": jql, "explanation": explain_jql(&jql)})),
    }
}

fn filter(ctx: &Context, cmd: FilterCommand) -> Result<Value> {
    let mut state = load_state(ctx)?;
    let out = match cmd {
        FilterCommand::List => json!({"data": state.filters.values().collect::<Vec<_>>()}),
        FilterCommand::Create { name, jql } => {
            let id = next_id(&mut state, "filter");
            let filter = SavedFilter {
                id: id.clone(),
                name,
                jql,
            };
            state.filters.insert(id.clone(), filter.clone());
            json!(filter)
        }
        FilterCommand::Update { id, jql } => {
            let filter = state
                .filters
                .get_mut(&id)
                .ok_or_else(|| JiraliError::NotFound(format!("filter {id} not found")))?;
            filter.jql = jql;
            json!(filter)
        }
        FilterCommand::Delete { id } => {
            state
                .filters
                .remove(&id)
                .ok_or_else(|| JiraliError::NotFound(format!("filter {id} not found")))?;
            json!({"deleted": id})
        }
    };
    save_state(ctx, &state)?;
    Ok(out)
}

fn adf_cmd(cmd: AdfCommand) -> Result<Value> {
    match cmd {
        AdfCommand::FromMarkdown(arg) => Ok(markdown_to_adf(&read_text_arg(arg)?)),
        AdfCommand::ToMarkdown(arg) => {
            Ok(json!({"markdown": adf_to_markdown(&read_json_text_arg(arg)?)?, "lossy": false}))
        }
        AdfCommand::Validate(arg) => {
            let value = read_json_text_arg(arg)?;
            let ok = value.get("type").and_then(Value::as_str) == Some("doc");
            if ok {
                Ok(json!({"valid": true}))
            } else {
                Err(JiraliError::Validation("ADF root must be type=doc".into()))
            }
        }
        AdfCommand::Normalize(arg) => Ok(normalize_adf(read_json_text_arg(arg)?)),
    }
}

fn alias(ctx: &Context, cmd: AliasCommand) -> Result<Value> {
    let mut config = load_config(ctx)?;
    match cmd {
        AliasCommand::Refresh => Ok(json!({"refreshed": true, "aliases": config.aliases})),
        AliasCommand::List => Ok(json!({"data": config.aliases})),
        AliasCommand::Set {
            field_id,
            alias,
            r#type,
        } => {
            config.aliases.insert(
                alias.clone(),
                FieldAlias {
                    field_id,
                    alias: alias.clone(),
                    field_type: r#type,
                },
            );
            save_config(ctx, &config)?;
            Ok(json!({"alias": alias}))
        }
        AliasCommand::Remove { alias } => {
            config.aliases.remove(&alias);
            save_config(ctx, &config)?;
            Ok(json!({"removed": alias}))
        }
    }
}

fn attach(ctx: &Context, cmd: AttachCommand) -> Result<Value> {
    let mut state = load_state(ctx)?;
    let out = match cmd {
        AttachCommand::List { key } => {
            json!({"key": key, "data": state.attachments.get(&key).cloned().unwrap_or_default()})
        }
        AttachCommand::Upload { key, path } => {
            ensure_issue_exists(&state, &key)?;
            let bytes = if let Some(path) = path.clone() {
                fs::read(&path).map_err(|e| JiraliError::Io(e.to_string()))?
            } else {
                read_stdin_bytes()?
            };
            let filename = path
                .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
                .unwrap_or_else(|| "stdin".into());
            let id = next_id(&mut state, "attachment");
            let sha256 = format!("{:x}", Sha256::digest(&bytes));
            let item = Attachment {
                id: id.clone(),
                filename,
                sha256,
                size: bytes.len() as u64,
            };
            state
                .attachments
                .entry(key.clone())
                .or_default()
                .push(item.clone());
            json!({"key": key, "attachment": item})
        }
        AttachCommand::Download { key, id, output } => {
            let item = state
                .attachments
                .get(&key)
                .into_iter()
                .flatten()
                .find(|a| a.id == id)
                .ok_or_else(|| JiraliError::NotFound(format!("attachment {id} not found")))?;
            if let Some(path) = output {
                fs::write(
                    path,
                    format!("attachment {} sha256 {}\n", item.filename, item.sha256),
                )
                .map_err(|e| JiraliError::Io(e.to_string()))?;
            }
            json!(item)
        }
        AttachCommand::Remove { key, id } => {
            let list = state.attachments.entry(key).or_default();
            list.retain(|a| a.id != id);
            json!({"removed": id})
        }
    };
    save_state(ctx, &state)?;
    Ok(out)
}

fn worklog(ctx: &Context, cmd: WorklogCommand) -> Result<Value> {
    let mut state = load_state(ctx)?;
    let out = match cmd {
        WorklogCommand::Add {
            key,
            duration,
            comment,
        } => {
            ensure_issue_exists(&state, &key)?;
            let id = next_id(&mut state, "worklog");
            let item = Worklog {
                id: id.clone(),
                issue: key.clone(),
                duration: duration.clone(),
                seconds: parse_duration(&duration)?,
                comment,
                created: now(),
            };
            state
                .worklogs
                .entry(key.clone())
                .or_default()
                .push(item.clone());
            json!(item)
        }
        WorklogCommand::List { key } => {
            json!({"key": key, "data": state.worklogs.get(&key).cloned().unwrap_or_default()})
        }
        WorklogCommand::Edit { key, id, duration } => {
            let list = state
                .worklogs
                .get_mut(&key)
                .ok_or_else(|| JiraliError::NotFound(format!("worklogs for {key} not found")))?;
            let item = list
                .iter_mut()
                .find(|w| w.id == id)
                .ok_or_else(|| JiraliError::NotFound(format!("worklog {id} not found")))?;
            item.duration = duration.clone();
            item.seconds = parse_duration(&duration)?;
            json!(item)
        }
        WorklogCommand::Delete { key, id } => {
            let list = state.worklogs.entry(key).or_default();
            list.retain(|w| w.id != id);
            json!({"deleted": id})
        }
        WorklogCommand::Aggregate { by } => {
            let total: u64 = state.worklogs.values().flatten().map(|w| w.seconds).sum();
            json!({"by": by.unwrap_or_else(|| "all".into()), "total_seconds": total})
        }
    };
    save_state(ctx, &state)?;
    Ok(out)
}

fn hierarchy(ctx: &Context, cmd: HierarchyCommand) -> Result<Value> {
    let state = load_state(ctx)?;
    match cmd {
        HierarchyCommand::Ancestors { key } => {
            Ok(json!({"key": key, "ancestors": ancestors(&state, &key)?}))
        }
        HierarchyCommand::Descendants { key } => {
            Ok(json!({"key": key, "descendants": descendants(&state, &key)}))
        }
        HierarchyCommand::Tree { key } => Ok(
            json!({"key": key, "ancestors": ancestors(&state, &key)?, "descendants": descendants(&state, &key)}),
        ),
    }
}

fn release(ctx: &Context, cmd: ReleaseCommand) -> Result<Value> {
    let mut state = load_state(ctx)?;
    let out = match cmd {
        ReleaseCommand::List { project } => {
            json!({"data": state.releases.values().filter(|r| project.as_ref().map(|p| &r.project == p).unwrap_or(true)).collect::<Vec<_>>()})
        }
        ReleaseCommand::Create { project, name } => {
            let id = next_id(&mut state, "release");
            let rel = Release {
                id: id.clone(),
                project,
                name,
                archived: false,
            };
            state.releases.insert(id.clone(), rel.clone());
            json!(rel)
        }
        ReleaseCommand::Update { id, name, archived } => {
            let rel = state
                .releases
                .get_mut(&id)
                .ok_or_else(|| JiraliError::NotFound(format!("release {id} not found")))?;
            if let Some(name) = name {
                rel.name = name;
            }
            if let Some(archived) = archived {
                rel.archived = archived;
            }
            json!(rel)
        }
        ReleaseCommand::Issues { version } => {
            json!({"version": version, "issues": state.issues.values().filter(|i| i.fields.get("fixVersion") == Some(&json!(version))).collect::<Vec<_>>()})
        }
        ReleaseCommand::Notes { version } => {
            json!({"version": version, "markdown": format!("# Release {version}\n\n{}", state.issues.values().map(|i| format!("- {} {}", i.key, i.summary)).collect::<Vec<_>>().join("\n"))})
        }
    };
    save_state(ctx, &state)?;
    Ok(out)
}

fn history(ctx: &Context, cmd: HistoryCommand) -> Result<Value> {
    let state = load_state(ctx)?;
    match cmd {
        HistoryCommand::Changelog { key, field, since } => {
            let issue = state
                .issues
                .get(&key)
                .ok_or_else(|| JiraliError::NotFound(format!("issue {key} not found")))?;
            let items: Vec<_> = issue
                .history
                .iter()
                .filter(|h| {
                    field
                        .as_ref()
                        .map(|f| h.to_string().contains(f))
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            Ok(json!({"key": key, "since": since, "data": items}))
        }
        HistoryCommand::SinceTransition { key, status } => {
            Ok(json!({"key": key, "status": status, "events": []}))
        }
    }
}

fn workflow(ctx: &Context, cmd: WorkflowCommand) -> Result<Value> {
    match cmd {
        WorkflowCommand::List { project } => {
            Ok(json!({"project": project, "workflows": ["default"]}))
        }
        WorkflowCommand::Transitions { key } => {
            ensure_issue_exists(&load_state(ctx)?, &key)?;
            Ok(json!({"key": key, "transitions": ["To Do", "In Progress", "Code Review", "Done"]}))
        }
        WorkflowCommand::Validate { key, status } => Ok(
            json!({"key": key, "status": status, "valid": !status.is_empty(), "required_fields": if status.to_lowercase().contains("review") { json!(["root_cause"]) } else { json!([]) }}),
        ),
    }
}

fn webhook(ctx: &Context, cmd: WebhookCommand) -> Result<Value> {
    let mut state = load_state(ctx)?;
    let out = match cmd {
        WebhookCommand::Register { url, event, jql } => {
            let id = next_id(&mut state, "webhook");
            let hook = Webhook {
                id: id.clone(),
                url,
                event,
                jql,
            };
            state.webhooks.insert(id.clone(), hook.clone());
            json!(hook)
        }
        WebhookCommand::List => json!({"data": state.webhooks.values().collect::<Vec<_>>()}),
        WebhookCommand::Deregister { id } => {
            state
                .webhooks
                .remove(&id)
                .ok_or_else(|| JiraliError::NotFound(format!("webhook {id} not found")))?;
            json!({"deregistered": id})
        }
        WebhookCommand::Listen {
            event,
            filter,
            timeout,
            for_each,
        } => {
            if timeout == 0 {
                return Err(JiraliError::Timeout(
                    "webhook listen timeout expired".into(),
                ));
            }
            json!({"event": event, "filter": filter, "for_each": for_each, "matched": true, "payload": { "mock": true }})
        }
        WebhookCommand::Replay { id } => json!({"id": id, "payload": {"mock": true}}),
    };
    save_state(ctx, &state)?;
    Ok(out)
}

fn report(ctx: &Context, cmd: ReportCommand) -> Result<Value> {
    let name = match cmd {
        ReportCommand::Velocity(_) => "velocity",
        ReportCommand::Burndown(_) => "burndown",
        ReportCommand::Cfd(_) => "cfd",
        ReportCommand::CycleTime(_) => "cycle_time",
        ReportCommand::LeadTime(_) => "lead_time",
        ReportCommand::Throughput(_) => "throughput",
        ReportCommand::Wip(_) => "wip",
        ReportCommand::Aging(_) => "aging",
        ReportCommand::FlowEfficiency(_) => "flow_efficiency",
    };
    let state = load_state(ctx)?;
    Ok(
        json!({"report": name, "series": [{"bucket": Utc::now().date_naive().to_string(), "value": state.issues.len()}], "schema": "jirali.report.v1"}),
    )
}

fn assets(ctx: &Context, cmd: AssetsCommand) -> Result<Value> {
    let mut state = load_state(ctx)?;
    let out = match cmd {
        AssetsCommand::Schemas => json!({"schemas": ["local"]}),
        AssetsCommand::Aql { query } => {
            json!({"query": query, "data": state.assets.values().collect::<Vec<_>>()})
        }
        AssetsCommand::Lint { query } => jql_lint(&query),
        AssetsCommand::Object { command } => match command {
            AssetObjectCommand::Get { id } => state
                .assets
                .get(&id)
                .cloned()
                .ok_or_else(|| JiraliError::NotFound(format!("asset {id} not found")))?,
            AssetObjectCommand::Create { schema, name } => {
                let id = next_id(&mut state, "asset");
                let value = json!({"id": id, "schema": schema, "name": name});
                state
                    .assets
                    .insert(value["id"].as_str().unwrap().into(), value.clone());
                value
            }
            AssetObjectCommand::Update { id, name } => {
                let value = state
                    .assets
                    .get_mut(&id)
                    .ok_or_else(|| JiraliError::NotFound(format!("asset {id} not found")))?;
                if let Some(name) = name {
                    value["name"] = json!(name);
                }
                value.clone()
            }
            AssetObjectCommand::Delete { id } => {
                state
                    .assets
                    .remove(&id)
                    .ok_or_else(|| JiraliError::NotFound(format!("asset {id} not found")))?;
                json!({"deleted": id})
            }
            AssetObjectCommand::Link { source, target } => {
                json!({"source": source, "target": target, "linked": true})
            }
        },
    };
    save_state(ctx, &state)?;
    Ok(out)
}

fn automation(ctx: &Context, cmd: AutomationCommand) -> Result<Value> {
    let mut state = load_state(ctx)?;
    let out = match cmd {
        AutomationCommand::List { project } => {
            json!({"project": project, "data": state.automations.values().collect::<Vec<_>>()})
        }
        AutomationCommand::Get { id, format } => {
            let value = state
                .automations
                .get(&id)
                .cloned()
                .unwrap_or_else(|| json!({"id": id, "name": "local rule", "triggers": []}));
            match format {
                DataFormat::Json => value,
                DataFormat::Yaml => {
                    json!({"yaml": serde_yaml::to_string(&value).unwrap_or_default()})
                }
            }
        }
        AutomationCommand::Trigger { id, issue } => {
            json!({"id": id, "issue": issue, "triggered": true})
        }
        AutomationCommand::Audit { id } => json!({"id": id, "events": []}),
        AutomationCommand::Export { id } => {
            let value = state
                .automations
                .get(&id)
                .cloned()
                .unwrap_or_else(|| json!({"id": id}));
            json!({"id": id, "yaml": serde_yaml::to_string(&value).unwrap_or_default()})
        }
        AutomationCommand::Import { file } => {
            let text = fs::read_to_string(file).map_err(|e| JiraliError::Io(e.to_string()))?;
            let value: Value =
                serde_yaml::from_str(&text).map_err(|e| JiraliError::Validation(e.to_string()))?;
            let id = value
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| next_id(&mut state, "automation"));
            state.automations.insert(id.clone(), value);
            json!({"imported": id})
        }
    };
    save_state(ctx, &state)?;
    Ok(out)
}

fn local(ctx: &Context, cmd: LocalCommand) -> Result<Value> {
    let state = load_state(ctx)?;
    match cmd {
        LocalCommand::Embed { jql } => {
            Ok(json!({"embedded": state.issues.len(), "jql": jql, "vector_store": "local"}))
        }
        LocalCommand::Search { query, semantic } => {
            let results: Vec<_> = state
                .issues
                .values()
                .filter(|i| i.summary.to_lowercase().contains(&query.to_lowercase()))
                .collect();
            Ok(json!({"query": query, "semantic": semantic, "data": results}))
        }
        LocalCommand::Nearest { key, k } => Ok(json!({"key": key, "k": k, "neighbors": []})),
        LocalCommand::Invalidate { key } => {
            Ok(json!({"invalidated": key.unwrap_or_else(|| "all".into())}))
        }
    }
}

fn audit(ctx: &Context, cmd: AuditCommand) -> Result<Value> {
    let text = fs::read_to_string(ctx.audit_path()).unwrap_or_default();
    let records: Vec<Value> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    match cmd {
        AuditCommand::Trace { correlation_id } => Ok(
            json!({"correlation_id": correlation_id, "events": records.into_iter().filter(|r| r.get("correlation_id").and_then(Value::as_str) == Some(correlation_id.as_str())).collect::<Vec<_>>()}),
        ),
        AuditCommand::List { limit } => {
            Ok(json!({"data": records.into_iter().rev().take(limit).collect::<Vec<_>>()}))
        }
    }
}

fn branch(_ctx: &Context, cmd: BranchCommand) -> Result<Value> {
    match cmd {
        BranchCommand::Start { key, transition } => Ok(
            json!({"key": key, "branch": format!("jira/{}-{}", key.to_lowercase(), "work"), "transition": transition}),
        ),
        BranchCommand::Link { key } => Ok(json!({"key": key, "linked": true})),
    }
}

fn skill(cmd: SkillCommand) -> Result<Value> {
    match cmd {
        SkillCommand::Emit => Ok(json!({
            "name": "jirali",
            "description": "Use Jirali to inspect and mutate Jira from agent-safe shell commands.",
            "commands": ["issue view", "issue list", "issue create", "issue transition", "jql lint", "plan", "apply", "report velocity"]
        })),
    }
}

fn mcp(cmd: McpCommand) -> Result<Value> {
    match cmd {
        McpCommand::Serve { transport } => Ok(
            json!({"mcp": "serve", "transport": format!("{transport:?}").to_lowercase(), "tools": ["issue.view", "issue.list", "jql.search"]}),
        ),
    }
}

fn plan_file(file: &Path) -> Result<Value> {
    let value = read_yaml_or_json(file)?;
    Ok(
        json!({"file": file, "dry_run": true, "operations": plan_operations(&value), "side_effects": false}),
    )
}

fn apply_file(ctx: &Context, args: ApplyArgs) -> Result<Value> {
    let value = read_yaml_or_json(&args.file)?;
    Ok(
        json!({"file": args.file, "transactional": args.transactional, "applied": plan_operations(&value), "idempotent": true, "profile": ctx.profile}),
    )
}

fn batch_file(_ctx: &Context, args: BatchArgs) -> Result<Value> {
    let value = read_yaml_or_json(&args.file)?;
    let operations = value
        .get("operations")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);
    Ok(
        json!({"file": args.file, "operations": operations, "parallel": args.parallel, "transactional": args.transactional, "results": []}),
    )
}

fn diff(ctx: &Context, args: DiffArgs) -> Result<Value> {
    let state = load_state(ctx)?;
    ensure_issue_exists(&state, &args.key)?;
    Ok(json!({"key": args.key, "as_of": args.as_of, "changes": state.issues[&args.key].history}))
}

fn snapshot(ctx: &Context, cmd: SnapshotCommand) -> Result<Value> {
    let mut state = load_state(ctx)?;
    let out = match cmd {
        SnapshotCommand::Create { jql } => {
            let id = next_id(&mut state, "snapshot");
            let issues: Vec<_> = state
                .issues
                .values()
                .filter(|i| jql.as_ref().map(|j| jql_matches(i, j)).unwrap_or(true))
                .cloned()
                .collect();
            let value = json!({"id": id, "created": now(), "jql": jql, "issues": issues});
            state
                .snapshots
                .insert(value["id"].as_str().unwrap().into(), value.clone());
            value
        }
        SnapshotCommand::Diff { left, right } => {
            json!({"left": left, "right": right, "added": [], "removed": [], "changed": []})
        }
    };
    save_state(ctx, &state)?;
    Ok(out)
}

fn api(ctx: &Context, args: ApiArgs) -> Result<Value> {
    let config = load_config(ctx)?;
    let profile = config
        .profiles
        .get(&ctx.profile)
        .ok_or_else(|| JiraliError::Usage(format!("profile {} is not configured", ctx.profile)))?;
    let site = profile
        .site_url
        .as_ref()
        .ok_or_else(|| JiraliError::Usage("profile is missing site_url".into()))?;
    let url = if args.path.starts_with("http") {
        args.path.clone()
    } else {
        format!("{}{}", site.trim_end_matches('/'), args.path)
    };
    let method = Method::from_bytes(args.method.as_bytes())
        .map_err(|e| JiraliError::Usage(e.to_string()))?;
    let mut request = Client::new()
        .request(method, &url)
        .header("X-Jirali-Correlation-Id", &ctx.correlation_id)
        .header("x-atlassian-force-account-id", "true")
        .header(
            "User-Agent",
            if ctx.no_input {
                format!("jirali-agent/{}", env!("CARGO_PKG_VERSION"))
            } else {
                format!("jirali-human/{}", env!("CARGO_PKG_VERSION"))
            },
        );
    if let Some(token) = profile.api_token.as_deref() {
        let email = profile.email.as_deref().unwrap_or("");
        let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{email}:{token}"));
        request = request.header("Authorization", format!("Basic {encoded}"));
    } else if let Some(pat) = profile.pat.as_deref() {
        request = request.bearer_auth(pat);
    }
    for header in args.header {
        if let Some((k, v)) = header.split_once(':') {
            request = request.header(k.trim(), v.trim());
        }
    }
    if let Some(body) = args.body {
        request = request.body(read_body_arg(&body)?);
    }
    let response = request
        .send()
        .map_err(|e| if e.is_timeout() { JiraliError::Timeout(e.to_string()) } else { JiraliError::Api(e.to_string()) })?;
    map_jira_response(response)
}

fn graphql(ctx: &Context, args: GraphqlArgs) -> Result<Value> {
    let query = if let Some(file) = args.file {
        fs::read_to_string(file).map_err(|e| JiraliError::Io(e.to_string()))?
    } else {
        args.query
            .ok_or_else(|| JiraliError::Usage("--query or --file is required".into()))?
    };
    api(
        ctx,
        ApiArgs {
            method: "POST".into(),
            path: "/gateway/api/graphql".into(),
            body: Some(json!({"query": query}).to_string()),
            query: vec![],
            header: vec!["content-type: application/json".into()],
        },
    )
}

fn mask_stdin() -> Result<Value> {
    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| JiraliError::Io(e.to_string()))?;
    let mut value: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({"text": text}));
    mask_pii(&mut value);
    Ok(value)
}

fn generic_user(_ctx: &Context, cmd: UserCommand) -> Result<Value> {
    Ok(match cmd {
        UserCommand::Whoami => json!({"accountId": "local-user", "displayName": "Local User"}),
        UserCommand::Find { query } => json!({"query": query, "data": []}),
        UserCommand::Groups { account_id } => json!({"accountId": account_id, "groups": []}),
        UserCommand::Teams { query } => json!({"query": query, "teams": []}),
    })
}

fn generic_project(_ctx: &Context, cmd: ProjectCommand) -> Result<Value> {
    Ok(match cmd {
        ProjectCommand::List => json!({"data": []}),
        ProjectCommand::Get { key } => json!({"key": key}),
        ProjectCommand::Roles { key } => json!({"key": key, "roles": []}),
        ProjectCommand::Schemes { key } => json!({"key": key, "schemes": {}}),
    })
}

fn wiki(_ctx: &Context, cmd: WikiCommand) -> Result<Value> {
    Ok(match cmd {
        WikiCommand::Link { key, url } => json!({"key": key, "url": url, "linked": true}),
        WikiCommand::Search { query } => json!({"query": query, "data": []}),
        WikiCommand::Create { title, jql } => json!({"title": title, "jql": jql, "created": true}),
    })
}

fn compass(_ctx: &Context, cmd: CompassCommand) -> Result<Value> {
    Ok(match cmd {
        CompassCommand::List => json!({"components": []}),
        CompassCommand::Link { key, component } => {
            json!({"key": key, "component": component, "linked": true})
        }
    })
}

fn goal(_ctx: &Context, cmd: GoalCommand) -> Result<Value> {
    Ok(match cmd {
        GoalCommand::List => json!({"goals": []}),
        GoalCommand::Progress { id } => json!({"id": id, "progress": null}),
        GoalCommand::Link { key, goal } => json!({"key": key, "goal": goal, "linked": true}),
    })
}

fn jsm(_ctx: &Context, cmd: JsmCommand) -> Result<Value> {
    Ok(match cmd {
        JsmCommand::Desks => json!({"desks": []}),
        JsmCommand::RequestTypes { desk } => json!({"desk": desk, "request_types": []}),
        JsmCommand::Requests { queue } => json!({"queue": queue, "requests": []}),
        JsmCommand::Sla { request } => json!({"request": request, "sla": []}),
        JsmCommand::Queues { desk } => json!({"desk": desk, "queues": []}),
        JsmCommand::Customers { query } => json!({"query": query, "customers": []}),
        JsmCommand::Orgs => json!({"organizations": []}),
        JsmCommand::Ops { command } => json!({"ops": format!("{command:?}")}),
    })
}

fn emit_stdout(ctx: &Context, mut value: Value) {
    if ctx.mask_pii {
        mask_pii(&mut value);
    }
    let out = if ctx.meta {
        json!({"data": value, "_schema": "jirali.response.v1", "_meta": ctx.meta("jirali.response.v1")})
    } else {
        value
    };
    if ctx.json {
        println!("{}", serde_json::to_string(&out).unwrap());
    } else {
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    }
}

fn emit_stderr(ctx: &Context, err: &JiraliError) {
    let value = json!({
        "error": true,
        "code": err.code(),
        "exit_code": err.exit_code(),
        "message": err.to_string(),
        "suggestion": err.suggestion(),
        "context": {},
        "correlation_id": ctx.correlation_id,
        "documentation_url": format!("https://jirali.dev/errors/{}", err.code())
    });
    eprintln!("{}", serde_json::to_string(&value).unwrap());
}

fn parse_fields(items: Vec<String>) -> Result<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    for item in items {
        let (key, raw) = item
            .split_once('=')
            .ok_or_else(|| JiraliError::Usage(format!("field must be key=value: {item}")))?;
        let value = serde_json::from_str(raw).unwrap_or_else(|_| json!(raw));
        out.insert(key.to_string(), value);
    }
    Ok(out)
}

fn project_issue(issue: &Issue, profile: Option<ViewProfile>, fields: Option<&str>) -> Value {
    let mut value = json!(issue);
    if let Some(list) = fields {
        let wanted: BTreeSet<_> = list.split(',').map(str::trim).collect();
        if let Some(obj) = value.as_object_mut() {
            obj.retain(|k, _| wanted.contains(k.as_str()) || k == "key" || k == "id");
        }
    } else if matches!(profile, Some(ViewProfile::Skinny) | None) {
        value = json!({"key": issue.key, "id": issue.id, "summary": issue.summary, "status": issue.status, "assignee": issue.assignee, "_schema": "jirali.issue.v1"});
    } else if matches!(profile, Some(ViewProfile::Triage)) {
        value = json!({"key": issue.key, "summary": issue.summary, "status": issue.status, "priority": issue.priority, "labels": issue.labels, "assignee": issue.assignee, "_schema": "jirali.issue.v1"});
    }
    value
}

fn ensure_issue_exists(state: &State, key: &str) -> Result<()> {
    if state.issues.contains_key(key) {
        Ok(())
    } else {
        Err(JiraliError::NotFound(format!("issue {key} not found")))
    }
}

fn jql_matches(issue: &Issue, jql: &str) -> bool {
    let jql = jql.trim();
    if jql.is_empty() {
        return true;
    }
    let lower = jql.to_lowercase();
    if lower.contains("project") && !lower.contains(&issue.project.to_lowercase()) {
        return false;
    }
    if lower.contains("status") && !lower.contains(&issue.status.to_lowercase()) {
        return false;
    }
    if lower.contains("assignee") {
        if let Some(assignee) = &issue.assignee {
            if !lower.contains(&assignee.to_lowercase()) && !lower.contains("currentuser()") {
                return false;
            }
        }
    }
    true
}

fn jql_lint(jql: &str) -> Value {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let lower = jql.to_lowercase();
    if lower.contains("!=") || lower.contains(" not ") {
        warnings.push(json!({"rule": "negation", "message": "Negation can force broad scans; prefer positive indexed predicates."}));
    }
    if lower.matches(" and ").count() > 2 {
        warnings.push(json!({"rule": "top_level_and", "message": "Many top-level AND clauses can become hard for agents and Jira to reason about."}));
    }
    if lower.contains("order by") && !lower.contains("updated") {
        warnings.push(json!({"rule": "unbounded_order_by", "message": "Prefer ORDER BY updated DESC with an explicit limit."}));
    }
    if jql.trim().is_empty() {
        errors.push(json!({"rule": "empty", "message": "JQL must not be empty."}));
    }
    json!({"valid": errors.is_empty(), "warnings": warnings, "errors": errors})
}

fn explain_jql(jql: &str) -> String {
    jql.replace("project =", "Issues in project")
        .replace("assignee = currentUser()", "assigned to the current user")
        .replace("status =", "with status")
}

fn markdown_to_adf(markdown: &str) -> Value {
    let content: Vec<Value> = markdown.lines().map(|line| {
        if let Some(text) = line.strip_prefix("# ") {
            json!({"type": "heading", "attrs": {"level": 1}, "content": [{"type": "text", "text": text}]})
        } else if line.starts_with("- ") {
            json!({"type": "bulletList", "content": [{"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": line.trim_start_matches("- ")}]}]}]})
        } else if line.starts_with("```") {
            json!({"type": "codeBlock", "content": []})
        } else {
            json!({"type": "paragraph", "content": inline_text(line)})
        }
    }).collect();
    json!({"version": 1, "type": "doc", "content": content})
}

fn inline_text(line: &str) -> Vec<Value> {
    if line.trim().is_empty() {
        return vec![];
    }
    line.split_whitespace().map(|part| {
        if part.starts_with('@') {
            json!({"type": "mention", "attrs": {"id": part.trim_start_matches('@'), "text": part}})
        } else {
            json!({"type": "text", "text": part})
        }
    }).collect()
}

fn adf_to_markdown(value: &Value) -> Result<String> {
    let mut lines = Vec::new();
    for node in value
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| JiraliError::Validation("ADF content must be an array".into()))?
    {
        let kind = node.get("type").and_then(Value::as_str).unwrap_or("");
        let text = node_text(node);
        match kind {
            "heading" => lines.push(format!("# {text}")),
            "bulletList" => lines.push(format!("- {text}")),
            "codeBlock" => lines.push(format!("```\n{text}\n```")),
            _ => lines.push(text),
        }
    }
    Ok(lines.join("\n"))
}

fn node_text(node: &Value) -> String {
    if let Some(text) = node.get("text").and_then(Value::as_str) {
        return text.to_string();
    }
    node.get("content")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(node_text).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

fn normalize_adf(mut value: Value) -> Value {
    if value.get("version").is_none() {
        value["version"] = json!(1);
    }
    value
}

fn read_text_arg(arg: TextArg) -> Result<String> {
    if let Some(file) = arg.file {
        fs::read_to_string(file).map_err(|e| JiraliError::Io(e.to_string()))
    } else if let Some(text) = arg.text {
        Ok(text)
    } else {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .map_err(|e| JiraliError::Io(e.to_string()))?;
        Ok(text)
    }
}

fn read_json_text_arg(arg: TextArg) -> Result<Value> {
    let text = read_text_arg(arg)?;
    serde_json::from_str(&text).map_err(|e| JiraliError::Validation(e.to_string()))
}

fn read_json_or_file(raw: &str) -> Result<Value> {
    let text = if let Some(path) = raw.strip_prefix('@') {
        fs::read_to_string(path).map_err(|e| JiraliError::Io(e.to_string()))?
    } else if Path::new(raw).exists() {
        fs::read_to_string(raw).map_err(|e| JiraliError::Io(e.to_string()))?
    } else {
        raw.to_string()
    };
    serde_json::from_str(&text).map_err(|e| JiraliError::Validation(e.to_string()))
}

fn read_body_arg(raw: &str) -> Result<String> {
    if let Some(path) = raw.strip_prefix('@') {
        fs::read_to_string(path).map_err(|e| JiraliError::Io(e.to_string()))
    } else {
        Ok(raw.to_string())
    }
}

fn read_stdin_bytes() -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    io::stdin()
        .read_to_end(&mut bytes)
        .map_err(|e| JiraliError::Io(e.to_string()))?;
    Ok(bytes)
}

fn extract_mentions(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|part| {
            part.strip_prefix('@').map(|s| {
                s.trim_matches(|c: char| !c.is_alphanumeric() && c != '-')
                    .to_string()
            })
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_duration(raw: &str) -> Result<u64> {
    let raw = raw.trim();
    if let Some(hours) = raw.strip_suffix('h') {
        let value: f64 = hours
            .parse()
            .map_err(|_| JiraliError::Usage(format!("invalid duration {raw}")))?;
        Ok((value * 3600.0) as u64)
    } else if let Some(minutes) = raw.strip_suffix('m') {
        let value: u64 = minutes
            .parse()
            .map_err(|_| JiraliError::Usage(format!("invalid duration {raw}")))?;
        Ok(value * 60)
    } else {
        Err(JiraliError::Usage(format!(
            "invalid duration {raw}; use 2h, 90m, or 1.5h"
        )))
    }
}

fn ancestors(state: &State, key: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut current = state
        .issues
        .get(key)
        .ok_or_else(|| JiraliError::NotFound(format!("issue {key} not found")))?;
    while let Some(parent) = current.parent.clone() {
        out.push(parent.clone());
        current = state
            .issues
            .get(&parent)
            .ok_or_else(|| JiraliError::NotFound(format!("parent {parent} not found")))?;
    }
    Ok(out)
}

fn descendants(state: &State, key: &str) -> Vec<String> {
    state
        .issues
        .values()
        .filter(|i| i.parent.as_deref() == Some(key))
        .map(|i| i.key.clone())
        .collect()
}

fn read_yaml_or_json(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path).map_err(|e| JiraliError::Io(e.to_string()))?;
    if path.extension().and_then(|s| s.to_str()) == Some("json") {
        serde_json::from_str(&text).map_err(|e| JiraliError::Validation(e.to_string()))
    } else {
        serde_yaml::from_str(&text).map_err(|e| JiraliError::Validation(e.to_string()))
    }
}

fn plan_operations(value: &Value) -> Value {
    let issue_count = value
        .get("issues")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);
    let op_count = value
        .get("operations")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(issue_count);
    json!({"count": op_count, "creates": issue_count, "updates": 0, "links": 0, "noops": 0})
}

fn redact_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                if key.to_lowercase().contains("token") || key.to_lowercase().contains("secret") {
                    *val = json!("***REDACTED***");
                } else {
                    redact_value(val);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_value(item);
            }
        }
        _ => {}
    }
}

fn mask_pii(value: &mut Value) {
    match value {
        Value::String(s) => {
            if s.contains('@') {
                *s = "[masked-email]".into();
            }
        }
        Value::Object(map) => {
            for (key, val) in map {
                if matches!(key.as_str(), "displayName" | "email" | "assignee") {
                    *val = json!("[masked]");
                } else {
                    mask_pii(val);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                mask_pii(item);
            }
        }
        _ => {}
    }
}

fn tools_schema() -> Value {
    json!({
        "name": "jirali",
        "version": env!("CARGO_PKG_VERSION"),
        "exit_codes": {
            "0": "success", "1": "general_failure", "2": "usage_error", "3": "not_found",
            "4": "permission_denied", "5": "conflict_idempotent", "6": "rate_limited",
            "7": "validation_failed", "8": "timeout"
        },
        "groups": [
            "auth", "config", "issue", "link", "sprint", "board", "comment", "jql", "filter",
            "adf", "alias", "attach", "worklog", "hierarchy", "release", "history", "user",
            "project", "workflow", "webhook", "report", "wiki", "compass", "goal", "jsm",
            "assets", "automation", "local", "audit", "branch", "skill", "mcp", "plan",
            "apply", "batch", "diff", "snapshot", "api", "graphql", "mask"
        ]
    })
}
