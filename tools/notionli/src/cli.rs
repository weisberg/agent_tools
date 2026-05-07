use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

const DEFAULT_API_VERSION: &str = "2026-03-11";

#[derive(Debug, Parser)]
#[command(
    name = "notionli",
    version,
    about = "Notion for agents, scripts, and power users"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,

    /// Force JSON output.
    #[arg(long, global = true)]
    pub(crate) json: bool,

    /// Emit newline-delimited JSON for streamable commands.
    #[arg(long, global = true)]
    pub(crate) jsonl: bool,

    /// Output format override, e.g. json, md, agent-safe, table.
    #[arg(long, global = true)]
    pub(crate) format: Option<String>,

    /// Print only the primary ID when a command has one.
    #[arg(long, global = true)]
    pub(crate) quiet: bool,

    /// Execute writes. Without this, writes are dry-run plans.
    #[arg(long, global = true)]
    pub(crate) apply: bool,

    /// Explicit dry-run/plan mode for writes.
    #[arg(long, alias = "plan", global = true)]
    pub(crate) dry_run: bool,

    /// Active profile name.
    #[arg(long, global = true, default_value = "default")]
    pub(crate) profile: String,

    /// Override the Notion API version header.
    #[arg(long, global = true, default_value = DEFAULT_API_VERSION)]
    pub(crate) api_version: String,

    /// Retry network/API requests this many times.
    #[arg(long, global = true, default_value_t = 3)]
    pub(crate) retry: u32,

    /// Use a config/state root instead of ~/.local/share/notionli.
    #[arg(long, global = true)]
    pub(crate) home: Option<PathBuf>,

    /// Secret injection command. The command's stdout is used as the token.
    #[arg(long, global = true)]
    pub(crate) token_cmd: Option<String>,

    /// Pick the best candidate when resolution is ambiguous.
    #[arg(long, global = true)]
    pub(crate) pick_first: bool,

    /// Enforce a JSON policy file for this invocation.
    #[arg(long, global = true)]
    pub(crate) policy: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Authentication and credential checks.
    #[command(subcommand)]
    Auth(AuthCommand),
    /// Profile state.
    #[command(subcommand)]
    Profile(ProfileCommand),
    /// Config inspection.
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Health checks.
    #[command(subcommand)]
    Doctor(DoctorCommand),
    /// Resolve an alias, URL, UUID, title query, or selected target.
    Resolve(ResolveArgs),
    /// Alias management.
    #[command(subcommand)]
    Alias(AliasCommand),
    /// Persist the current target as '.'.
    Select { target: String },
    /// Show the selected target.
    Selected,
    /// Search Notion or the local cache.
    Search(SearchArgs),
    /// List children for a target.
    Ls(TreeArgs),
    /// Print a child tree for a target.
    Tree(TreeArgs),
    /// Open a target URL in a browser.
    Open { target: String },
    /// Page commands.
    #[command(subcommand)]
    Page(PageCommand),
    /// Block commands.
    #[command(subcommand)]
    Block(BlockCommand),
    /// Database container commands.
    #[command(subcommand)]
    Db(DbCommand),
    /// Data source commands.
    #[command(subcommand)]
    Ds(DsCommand),
    /// Row/page-in-data-source commands.
    #[command(subcommand)]
    Row(RowCommand),
    /// Comment commands.
    #[command(subcommand)]
    Comment(CommentCommand),
    /// User commands.
    #[command(subcommand)]
    User(UserCommand),
    /// Teamspace commands.
    #[command(subcommand)]
    Team(TeamCommand),
    /// File upload commands.
    #[command(subcommand)]
    File(FileCommand),
    /// Meeting-notes commands.
    #[command(subcommand)]
    Meeting(MeetingCommand),
    /// Webhook registration commands.
    #[command(subcommand)]
    Webhook(WebhookCommand),
    /// Direct-mode watch planning and cache polling.
    Watch(WatchArgs),
    /// Sync/cache commands.
    #[command(subcommand)]
    Sync(SyncCommand),
    /// Operation log commands.
    #[command(subcommand)]
    Op(OpCommand),
    /// Audit commands.
    #[command(subcommand)]
    Audit(AuditCommand),
    /// Policy commands.
    #[command(subcommand)]
    Policy(PolicyCommand),
    /// Batch operations.
    #[command(subcommand)]
    Batch(BatchCommand),
    /// Bulk workspace cleanup operations.
    #[command(subcommand)]
    Bulk(BulkCommand),
    /// Saved template commands.
    #[command(subcommand)]
    Template(TemplateCommand),
    /// Saved query commands.
    #[command(subcommand)]
    Query(QueryCommand),
    /// Workflow commands.
    #[command(subcommand)]
    Workflow(WorkflowCommand),
    /// Snapshot commands.
    #[command(subcommand)]
    Snapshot(SnapshotCommand),
    /// Local mock-server manifest commands.
    #[command(subcommand)]
    Mock(MockCommand),
    /// Fixture recording and replay commands.
    #[command(subcommand)]
    Fixture(FixtureCommand),
    /// Tool schema/introspection commands.
    #[command(subcommand)]
    Tools(ToolsCommand),
    /// MCP bridge commands.
    #[command(subcommand)]
    Mcp(McpCommand),
    /// CLI schema commands.
    #[command(subcommand)]
    Schema(SchemaCommand),
    /// Shell completions.
    Completion { shell: String },
    /// Future TUI entrypoint.
    Tui,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuthCommand {
    Login(AuthLoginArgs),
    #[command(subcommand)]
    Token(TokenCommand),
    Whoami,
    Doctor,
}

#[derive(Debug, Args)]
pub(crate) struct AuthLoginArgs {
    /// OAuth client id from the Notion public connection settings.
    #[arg(long)]
    pub(crate) client_id: Option<String>,

    /// OAuth client secret from the Notion public connection settings.
    #[arg(long)]
    pub(crate) client_secret: Option<String>,

    /// Full Notion OAuth authorization URL. Defaults to the standard authorize endpoint.
    #[arg(long)]
    pub(crate) auth_url: Option<String>,

    /// Redirect URI registered with the Notion public connection.
    #[arg(long)]
    pub(crate) redirect_uri: Option<String>,

    /// Complete login from an already received OAuth authorization code.
    #[arg(long)]
    pub(crate) code: Option<String>,

    /// Do not open a browser; print the authorization URL instead.
    #[arg(long)]
    pub(crate) no_browser: bool,

    /// Local callback port to use when redirect_uri is not supplied.
    #[arg(long, default_value_t = 53682)]
    pub(crate) port: u16,

    /// Seconds to wait for the browser callback.
    #[arg(long, default_value_t = 300)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TokenCommand {
    /// Store an integration token in the macOS keychain, or plaintext with --allow-plaintext.
    Set {
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        allow_plaintext: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProfileCommand {
    List,
    Create { name: String },
    Use { name: String },
    Show { name: Option<String> },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommand {
    Get { key: String },
    Set { key: String, value: String },
    UseProfile { overlay: String },
}

#[derive(Debug, Subcommand)]
pub(crate) enum DoctorCommand {
    RoundTrip { target: String },
    Cache,
    Api,
}

#[derive(Debug, Args)]
pub(crate) struct ResolveArgs {
    pub(crate) input: String,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AliasCommand {
    Set { name: String, reference: String },
    List,
    Remove { name: String },
}

#[derive(Debug, Args)]
pub(crate) struct SearchArgs {
    pub(crate) query: Option<String>,
    #[arg(long, value_enum)]
    pub(crate) r#type: Option<ObjectType>,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: u32,
    #[arg(long)]
    pub(crate) semantic: bool,
    #[arg(long)]
    pub(crate) recent: bool,
    #[arg(long)]
    pub(crate) stale: bool,
    #[arg(long)]
    pub(crate) orphaned: bool,
    #[arg(long)]
    pub(crate) duplicates: bool,
}

#[derive(Debug, Args)]
pub(crate) struct TreeArgs {
    pub(crate) target: String,
    #[arg(long, default_value_t = 1)]
    pub(crate) depth: u32,
}

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum ObjectType {
    Page,
    Database,
    Db,
    DataSource,
    Ds,
    Block,
    Comment,
    Row,
}

impl ObjectType {
    pub(crate) fn notion_value(&self) -> &'static str {
        match self {
            Self::Page | Self::Row => "page",
            Self::Database | Self::Db => "database",
            Self::DataSource | Self::Ds => "data_source",
            Self::Block => "block",
            Self::Comment => "comment",
        }
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum PageCommand {
    Get {
        target: String,
    },
    Fetch(PageFetchArgs),
    Section(PageSectionArgs),
    Outline(PageOutlineArgs),
    Create(PageCreateArgs),
    Update(PageUpdateArgs),
    Append(PageAppendArgs),
    Patch(PagePatchArgs),
    Rename(PageRenameArgs),
    Move(PageMoveArgs),
    Duplicate(PageDuplicateArgs),
    Trash(PageTrashArgs),
    Restore {
        target: String,
    },
    Edit {
        target: String,
        #[arg(long)]
        section: Option<String>,
        #[arg(long)]
        append_only: bool,
    },
    #[command(subcommand)]
    Worktree(PageWorktreeCommand),
    Todos {
        target: String,
    },
    Headings {
        target: String,
    },
    Links {
        target: String,
    },
    Mentions {
        target: String,
    },
    Files {
        target: String,
    },
    Comments {
        target: String,
        #[arg(long)]
        unresolved: bool,
    },
    CheckStale {
        target: String,
        #[arg(long)]
        max_age: String,
    },
}

#[derive(Debug, Args)]
pub(crate) struct PageFetchArgs {
    pub(crate) target: String,
    #[arg(long, default_value = "json")]
    pub(crate) format: String,
    #[arg(long)]
    pub(crate) budget: Option<u32>,
    #[arg(long, default_value = "full")]
    pub(crate) strategy: String,
    #[arg(long)]
    pub(crate) headings: Option<String>,
    #[arg(long)]
    pub(crate) omit: Option<String>,
    #[arg(long)]
    pub(crate) recursive: bool,
    #[arg(long)]
    pub(crate) out: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct PageSectionArgs {
    pub(crate) target: String,
    pub(crate) heading: String,
    #[arg(long, default_value = "md")]
    pub(crate) format: String,
    #[arg(long)]
    pub(crate) include_subsections: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PageOutlineArgs {
    pub(crate) target: String,
    #[arg(long)]
    pub(crate) with_block_ids: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PageCreateArgs {
    #[arg(long)]
    pub(crate) parent: String,
    #[arg(long)]
    pub(crate) title: Option<String>,
    #[arg(long)]
    pub(crate) md: Option<PathBuf>,
    #[arg(long)]
    pub(crate) body: Option<String>,
    #[arg(long)]
    pub(crate) template: Option<String>,
    #[arg(long = "set")]
    pub(crate) set: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct PageUpdateArgs {
    pub(crate) target: String,
    #[arg(long)]
    pub(crate) title: Option<String>,
    #[arg(long = "set")]
    pub(crate) set: Vec<String>,
    #[arg(long)]
    pub(crate) if_unmodified_since: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct PageAppendArgs {
    pub(crate) target: String,
    #[arg(long)]
    pub(crate) md: Option<PathBuf>,
    #[arg(long)]
    pub(crate) text: Option<String>,
    #[arg(long)]
    pub(crate) heading: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct PagePatchArgs {
    pub(crate) target: String,
    #[arg(long)]
    pub(crate) section: Option<String>,
    #[arg(long)]
    pub(crate) append_md: Option<PathBuf>,
    #[arg(long)]
    pub(crate) replace_md: Option<PathBuf>,
    #[arg(long)]
    pub(crate) prepend_md: Option<PathBuf>,
    #[arg(long)]
    pub(crate) append_text: Option<String>,
    #[arg(long)]
    pub(crate) op: Option<String>,
    #[arg(long)]
    pub(crate) heading: Option<String>,
    #[arg(long)]
    pub(crate) block: Option<String>,
    #[arg(long)]
    pub(crate) text: Option<String>,
    #[arg(long)]
    pub(crate) diff: bool,
    #[arg(long)]
    pub(crate) if_unmodified_since: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct PageRenameArgs {
    pub(crate) target: String,
    pub(crate) new_title: String,
}

#[derive(Debug, Args)]
pub(crate) struct PageMoveArgs {
    pub(crate) target: String,
    pub(crate) new_parent: String,
}

#[derive(Debug, Args)]
pub(crate) struct PageDuplicateArgs {
    pub(crate) target: String,
    #[arg(long)]
    pub(crate) to: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct PageTrashArgs {
    pub(crate) target: String,
    #[arg(long)]
    pub(crate) confirm_title: Option<String>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PageWorktreeCommand {
    Checkout {
        target: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Push {
        path: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum BlockCommand {
    Get {
        block_id: String,
    },
    Children {
        parent: String,
        #[arg(long, default_value_t = 1)]
        depth: u32,
    },
    Find {
        parent: String,
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        r#type: Option<String>,
        #[arg(long)]
        heading: Option<String>,
    },
    Append(BlockAppendArgs),
    Insert(BlockInsertArgs),
    Replace(BlockReplaceArgs),
    Update(BlockUpdateArgs),
    Move {
        block_id: String,
        #[arg(long)]
        after: String,
    },
    Trash {
        block_id: String,
    },
}

#[derive(Debug, Args)]
pub(crate) struct BlockAppendArgs {
    pub(crate) parent: String,
    #[arg(long)]
    pub(crate) md: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BlockInsertArgs {
    pub(crate) parent: String,
    #[arg(long)]
    pub(crate) position: String,
    #[arg(long)]
    pub(crate) md: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BlockReplaceArgs {
    pub(crate) block_id: String,
    #[arg(long)]
    pub(crate) text: Option<String>,
    #[arg(long)]
    pub(crate) md: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct BlockUpdateArgs {
    pub(crate) block_id: String,
    #[arg(long)]
    pub(crate) from: PathBuf,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DbCommand {
    List,
    Get { target: String },
}

#[derive(Debug, Subcommand)]
pub(crate) enum DsCommand {
    List {
        database: Option<String>,
    },
    Get {
        target: String,
    },
    Schema(DsSchemaArgs),
    Query(DsQueryArgs),
    BulkUpdate(DsBulkUpdateArgs),
    BulkArchive(DsBulkArchiveArgs),
    Deduplicate(DsDeduplicateArgs),
    Import(DsImportArgs),
    Export(DsExportArgs),
    Move {
        data_source: String,
        new_database: String,
    },
    Lint {
        target: String,
        #[arg(long)]
        rules: PathBuf,
    },
}

#[derive(Debug, Args)]
pub(crate) struct DsSchemaArgs {
    pub(crate) target: Option<String>,
    #[arg(long)]
    pub(crate) yaml: bool,
    #[arg(long)]
    pub(crate) json: bool,
    #[command(subcommand)]
    pub(crate) command: Option<DsSchemaCommand>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DsSchemaCommand {
    Diff {
        target: String,
        desired_file: PathBuf,
    },
    Apply {
        target: String,
        desired_file: PathBuf,
    },
    Validate {
        target: String,
        schema_file: PathBuf,
    },
}

#[derive(Debug, Args)]
pub(crate) struct DsQueryArgs {
    pub(crate) target: String,
    #[arg(long = "where")]
    pub(crate) where_clause: Option<String>,
    #[arg(long)]
    pub(crate) sort: Option<String>,
    #[arg(long)]
    pub(crate) filter: Option<String>,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: u32,
    #[arg(long)]
    pub(crate) expand: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct DsBulkUpdateArgs {
    pub(crate) target: String,
    #[arg(long = "where")]
    pub(crate) where_clause: Option<String>,
    #[arg(long = "set")]
    pub(crate) set: Vec<String>,
    #[arg(long)]
    pub(crate) max_write: Option<u32>,
}

#[derive(Debug, Args)]
pub(crate) struct DsBulkArchiveArgs {
    pub(crate) target: String,
    #[arg(long = "where")]
    pub(crate) where_clause: Option<String>,
    #[arg(long)]
    pub(crate) max_write: Option<u32>,
}

#[derive(Debug, Args)]
pub(crate) struct DsDeduplicateArgs {
    pub(crate) target: String,
    #[arg(long, default_value = "Name")]
    pub(crate) by: String,
    #[arg(long, default_value = "newest")]
    pub(crate) keep: String,
    #[arg(long)]
    pub(crate) max_write: Option<u32>,
}

#[derive(Debug, Args)]
pub(crate) struct DsImportArgs {
    pub(crate) target: String,
    #[arg(long)]
    pub(crate) csv: Option<PathBuf>,
    #[arg(long = "jsonl-file")]
    pub(crate) jsonl_file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) upsert_key: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct DsExportArgs {
    pub(crate) target: String,
    #[arg(long, default_value = "jsonl")]
    pub(crate) format: String,
    #[arg(long = "where")]
    pub(crate) where_clause: Option<String>,
    #[arg(long)]
    pub(crate) out: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum RowCommand {
    Get { target: String },
    Create(RowCreateArgs),
    Update(RowUpdateArgs),
    Upsert(RowUpsertArgs),
    Set(RowSetArgs),
    Relate(RowRelateArgs),
    Trash { target: String },
    Restore { target: String },
}

#[derive(Debug, Args)]
pub(crate) struct RowCreateArgs {
    pub(crate) ds: String,
    #[arg(long = "set")]
    pub(crate) set: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct RowUpdateArgs {
    pub(crate) target: String,
    #[arg(long = "set")]
    pub(crate) set: Vec<String>,
    #[arg(long)]
    pub(crate) if_unmodified_since: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct RowUpsertArgs {
    pub(crate) ds: String,
    #[arg(long)]
    pub(crate) key: String,
    #[arg(long = "set")]
    pub(crate) set: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct RowSetArgs {
    pub(crate) target: String,
    pub(crate) property: String,
    pub(crate) value: String,
}

#[derive(Debug, Args)]
pub(crate) struct RowRelateArgs {
    pub(crate) target: String,
    pub(crate) relation_prop: String,
    pub(crate) target_title: String,
    #[arg(long)]
    pub(crate) by_title: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CommentCommand {
    List {
        target: String,
        #[arg(long)]
        unresolved: bool,
    },
    Add(CommentAddArgs),
    Reply {
        discussion: String,
        #[arg(long)]
        text: String,
    },
    Resolve {
        comment_id: String,
    },
}

#[derive(Debug, Args)]
pub(crate) struct CommentAddArgs {
    #[arg(long)]
    pub(crate) page: Option<String>,
    #[arg(long)]
    pub(crate) block: Option<String>,
    #[arg(long)]
    pub(crate) text: String,
    #[arg(long)]
    pub(crate) mention_user: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum UserCommand {
    Me,
    List,
    Find { query: String },
}

#[derive(Debug, Subcommand)]
pub(crate) enum TeamCommand {
    List,
}

#[derive(Debug, Subcommand)]
pub(crate) enum FileCommand {
    Upload {
        path: PathBuf,
        #[arg(long)]
        multipart: bool,
    },
    Attach {
        path_or_id: String,
        #[arg(long)]
        page: Option<String>,
        #[arg(long)]
        block: Option<String>,
    },
    List,
    Status {
        file_upload_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum MeetingCommand {
    List {
        #[arg(long)]
        since: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    Get {
        block_id: String,
        #[arg(long)]
        summary: bool,
        #[arg(long)]
        transcript: bool,
        #[arg(long)]
        actions: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum WebhookCommand {
    List,
    Create(WebhookCreateArgs),
    Delete { webhook_id: String },
    Serve(WebhookServeArgs),
}

#[derive(Debug, Args)]
pub(crate) struct WebhookCreateArgs {
    #[arg(long, value_delimiter = ',')]
    pub(crate) events: Vec<String>,
    #[arg(long)]
    pub(crate) url: Option<String>,
    #[arg(long)]
    pub(crate) target: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct WebhookServeArgs {
    #[arg(long, default_value_t = 0)]
    pub(crate) port: u16,
    #[arg(long)]
    pub(crate) once: bool,
    #[arg(long)]
    pub(crate) out: Option<PathBuf>,
    #[arg(long)]
    pub(crate) on_event: Option<String>,
    #[arg(long)]
    pub(crate) secret: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct WatchArgs {
    pub(crate) target: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub(crate) events: Vec<String>,
    #[arg(long)]
    pub(crate) on_change: Option<String>,
    #[arg(long)]
    pub(crate) all_shared: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum SyncCommand {
    Run {
        #[arg(long)]
        full: bool,
        #[arg(long)]
        incremental: bool,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        all_shared: bool,
        #[arg(long)]
        mirror_to: Option<String>,
    },
    Status,
    Diff,
    Pull {
        #[arg(long)]
        since: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum OpCommand {
    List {
        #[arg(long, default_value_t = 20)]
        limit: u32,
        #[arg(long)]
        since: Option<String>,
    },
    Show {
        operation_id: String,
    },
    Undo {
        operation_id: String,
    },
    Status {
        operation_id: String,
    },
    Resume {
        operation_id: String,
    },
    Cancel {
        operation_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuditCommand {
    List,
    Show { operation_id: String },
}

#[derive(Debug, Subcommand)]
pub(crate) enum PolicyCommand {
    Show,
    Check {
        policy_file: PathBuf,
        command: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum BatchCommand {
    Apply {
        ops: PathBuf,
        #[arg(long)]
        continue_on_error: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum BulkCommand {
    Rename(BulkRenameArgs),
}

#[derive(Debug, Args)]
pub(crate) struct BulkRenameArgs {
    #[arg(long)]
    pub(crate) target: Option<String>,
    #[arg(long)]
    pub(crate) pattern: String,
    #[arg(long)]
    pub(crate) replace: String,
    #[arg(long)]
    pub(crate) max_write: Option<u32>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TemplateCommand {
    List,
    Register {
        name: String,
        #[arg(long)]
        from: PathBuf,
    },
    Apply {
        name: String,
        #[arg(long)]
        parent: String,
        #[arg(long = "set")]
        set: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum QueryCommand {
    Save {
        name: String,
        #[arg(long)]
        source: String,
        #[arg(long = "where")]
        where_clause: Option<String>,
        #[arg(long)]
        sort: Option<String>,
    },
    List,
    Run {
        name: String,
    },
    Show {
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum WorkflowCommand {
    List,
    Run {
        name: String,
        #[arg(long = "set")]
        set: Vec<String>,
    },
    Show {
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SnapshotCommand {
    Create {
        #[arg(long)]
        all_shared: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Diff {
        old_dir: PathBuf,
        new_dir: PathBuf,
    },
    RestorePage {
        page_id: String,
        #[arg(long)]
        from: PathBuf,
    },
    RestoreRow {
        row_id: String,
        #[arg(long)]
        from: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum MockCommand {
    Serve {
        #[arg(long, default_value_t = 0)]
        port: u16,
        #[arg(long)]
        once: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum FixtureCommand {
    Record {
        #[arg(long)]
        command: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Replay {
        file: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ToolsCommand {
    List,
    Schema {
        command: Option<String>,
        #[arg(long, default_value = "json-schema")]
        format: String,
        #[arg(long)]
        profile: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum McpCommand {
    Serve {
        #[arg(long)]
        stdio: bool,
        #[arg(long)]
        http: bool,
        #[arg(long, default_value_t = 0)]
        port: u16,
        #[arg(long)]
        once: bool,
        #[arg(long)]
        tool_profile: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SchemaCommand {
    Commands,
    Errors,
}
