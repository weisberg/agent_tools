use std::path::PathBuf;
use std::process;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::*;

pub fn main_entry() {
    let cli = Cli::parse();
    let wants_json = cli.json;

    match run(cli) {
        Ok(outcome) => emit_success(outcome, wants_json),
        Err(err) => {
            emit_error(&err, wants_json);
            process::exit(err.exit_code());
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "mdli",
    version,
    about = "Markdown document operations for agents"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,

    /// Emit a mdli/output/v1 JSON envelope.
    #[arg(long, global = true)]
    pub(crate) json: bool,

    /// Suppress non-error diagnostics.
    #[arg(long, global = true)]
    pub(crate) quiet: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Stable section ID operations.
    #[command(subcommand)]
    Id(IdCommand),
    /// Section operations.
    #[command(subcommand)]
    Section(SectionCommand),
    /// Markdown table operations.
    #[command(subcommand)]
    Table(TableCommand),
    /// Managed generated block operations.
    #[command(subcommand)]
    Block(BlockCommand),
    /// Frontmatter operations.
    #[command(subcommand)]
    Frontmatter(FrontmatterCommand),
    /// Lint mdli document invariants.
    Lint(LintArgs),
    /// Validate a document against a structural schema.
    Validate(ValidateArgs),
    /// Inspect sections, tables, blocks, and lint issues.
    Inspect(FileArgs),
    /// Render the document's heading hierarchy.
    Tree(FileArgs),
    /// Extract a bounded slice of section context for an agent.
    Context(ContextArgs),
    /// Render a template to stdout.
    #[command(subcommand)]
    Template(TemplateCommand),
    /// Validate an mdli recipe.
    #[command(subcommand)]
    Recipe(RecipeCommand),
    /// Apply a recipe to a document.
    Apply(ApplyArgs),
    /// Build a new document from a recipe.
    Build(BuildArgs),
    /// Emit a structural edit plan for a recipe.
    Plan(PlanArgs),
    /// Apply a previously-recorded edit plan.
    ApplyPlan(ApplyPlanArgs),
    /// Apply a JSON edit list atomically.
    Patch(PatchArgs),
    /// Compute a semantic diff between two Markdown documents.
    Diff(DiffArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ContextArgs {
    pub(crate) file: PathBuf,
    #[arg(long, conflicts_with = "path")]
    pub(crate) id: Option<String>,
    #[arg(long)]
    pub(crate) path: Option<String>,
    /// Soft cap on returned body size, expressed as approximate token count
    /// (4 characters per token). The body may be truncated at the nearest
    /// line boundary; metadata is always returned.
    #[arg(long, default_value_t = 2000)]
    pub(crate) max_tokens: usize,
    /// Include managed-block metadata for blocks inside the selected section.
    #[arg(long, default_value_t = true)]
    pub(crate) include_managed_blocks: bool,
    /// Include the rendered body content. Disable for metadata-only callers.
    #[arg(long, default_value_t = true)]
    pub(crate) include_body: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TemplateCommand {
    /// Render a template to stdout using prepared datasets.
    Render(TemplateRenderArgs),
}

#[derive(Debug, Args)]
pub(crate) struct TemplateRenderArgs {
    pub(crate) template: PathBuf,
    /// Bind a dataset NAME=PATH (NDJSON, JSON array, or scalar JSON value).
    #[arg(long = "data")]
    pub(crate) data: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum RecipeCommand {
    /// Validate a recipe schema.
    Validate(RecipeValidateArgs),
}

#[derive(Debug, Args)]
pub(crate) struct RecipeValidateArgs {
    pub(crate) recipe: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct ApplyArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) recipe: PathBuf,
    #[arg(long = "data")]
    pub(crate) data: Vec<String>,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct BuildArgs {
    #[arg(long)]
    pub(crate) recipe: PathBuf,
    #[arg(long = "data")]
    pub(crate) data: Vec<String>,
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long)]
    pub(crate) overwrite: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PlanArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) recipe: PathBuf,
    #[arg(long = "data")]
    pub(crate) data: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct ApplyPlanArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) plan: PathBuf,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct PatchArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) edits: PathBuf,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct DiffArgs {
    /// New (current) document.
    pub(crate) file: PathBuf,
    /// Old document to diff against.
    #[arg(long)]
    pub(crate) against: PathBuf,
    /// Render a human-readable text summary instead of JSON. Ignored when
    /// `--json` is also set.
    #[arg(long, default_value_t = false)]
    pub(crate) text: bool,
    /// Accepted for forward compatibility. mdli diff is always semantic.
    #[arg(long, default_value_t = false)]
    pub(crate) semantic: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum IdCommand {
    List(FileArgs),
    Assign(IdAssignArgs),
}

#[derive(Debug, Args)]
pub(crate) struct IdAssignArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) all: bool,
    #[arg(long)]
    pub(crate) section: Option<String>,
    #[arg(long)]
    pub(crate) id: Option<String>,
    #[arg(long)]
    pub(crate) auto: bool,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Subcommand)]
pub(crate) enum SectionCommand {
    List(FileArgs),
    Get(SectionGetArgs),
    Ensure(SectionEnsureArgs),
    Replace(SectionReplaceArgs),
    Delete(SectionSelectMutateArgs),
    Move(SectionMoveArgs),
    Rename(SectionRenameArgs),
}

#[derive(Debug, Args)]
pub(crate) struct SectionGetArgs {
    pub(crate) file: PathBuf,
    #[arg(long, conflicts_with = "path")]
    pub(crate) id: Option<String>,
    #[arg(long)]
    pub(crate) path: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct SectionEnsureArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) id: String,
    #[arg(long)]
    pub(crate) path: String,
    #[arg(long)]
    pub(crate) level: usize,
    #[arg(long)]
    pub(crate) after: Option<String>,
    #[arg(long)]
    pub(crate) before: Option<String>,
    #[arg(long)]
    pub(crate) enforce_path: bool,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct SectionReplaceArgs {
    pub(crate) file: PathBuf,
    #[arg(long, conflicts_with = "path")]
    pub(crate) id: Option<String>,
    #[arg(long)]
    pub(crate) path: Option<String>,
    #[arg(long, conflicts_with = "section_from_file")]
    pub(crate) body_from_file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) section_from_file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) managed: bool,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct SectionSelectMutateArgs {
    pub(crate) file: PathBuf,
    #[arg(long, conflicts_with = "path")]
    pub(crate) id: Option<String>,
    #[arg(long)]
    pub(crate) path: Option<String>,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct SectionMoveArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) id: String,
    #[arg(long, conflicts_with = "before")]
    pub(crate) after: Option<String>,
    #[arg(long)]
    pub(crate) before: Option<String>,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct SectionRenameArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) id: String,
    #[arg(long)]
    pub(crate) to: String,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TableCommand {
    List(FileArgs),
    Get(TableGetArgs),
    Replace(TableReplaceArgs),
    Upsert(TableUpsertArgs),
    DeleteRow(TableDeleteRowArgs),
    Sort(TableSortArgs),
    Fmt(TableFmtArgs),
}

#[derive(Debug, Args)]
pub(crate) struct TableGetArgs {
    pub(crate) file: PathBuf,
    #[arg(long, conflicts_with = "name")]
    pub(crate) section: Option<String>,
    #[arg(long)]
    pub(crate) name: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct TableReplaceArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) section: String,
    #[arg(long)]
    pub(crate) name: Option<String>,
    #[arg(long)]
    pub(crate) columns: String,
    #[arg(long)]
    pub(crate) from_rows: PathBuf,
    #[arg(long)]
    pub(crate) key: Option<String>,
    #[arg(long)]
    pub(crate) sort: Option<String>,
    #[arg(long, default_value = "error")]
    pub(crate) missing: MissingMode,
    #[arg(long, default_value = "error")]
    pub(crate) on_rich_cell: RichCellMode,
    #[arg(long, default_value = "error")]
    pub(crate) on_duplicate_key: DuplicateKeyMode,
    #[arg(long)]
    pub(crate) empty: Option<String>,
    #[arg(long = "link")]
    pub(crate) links: Vec<String>,
    #[arg(long = "truncate")]
    pub(crate) truncates: Vec<String>,
    #[arg(long)]
    pub(crate) escape_markdown: bool,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct TableUpsertArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) key: String,
    #[arg(long = "row")]
    pub(crate) rows: Vec<String>,
    #[arg(long)]
    pub(crate) from_rows: Option<PathBuf>,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct TableDeleteRowArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) key: String,
    #[arg(long)]
    pub(crate) value: String,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct TableSortArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) name: String,
    #[arg(long = "by")]
    pub(crate) by: String,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct TableFmtArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) all: bool,
    #[arg(long)]
    pub(crate) name: Option<String>,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum MissingMode {
    Empty,
    Error,
}

impl std::str::FromStr for MissingMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "empty" => Ok(Self::Empty),
            "error" => Ok(Self::Error),
            _ => Err("expected empty or error".to_string()),
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum RichCellMode {
    Error,
    Json,
    Truncate,
    Html,
}

impl std::str::FromStr for RichCellMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "error" => Ok(Self::Error),
            "json" => Ok(Self::Json),
            "truncate" => Ok(Self::Truncate),
            "html" => Ok(Self::Html),
            _ => Err("expected error, json, truncate, or html".to_string()),
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum DuplicateKeyMode {
    Error,
    First,
    Last,
}

impl std::str::FromStr for DuplicateKeyMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "error" => Ok(Self::Error),
            "first" => Ok(Self::First),
            "last" => Ok(Self::Last),
            _ => Err("expected error, first, or last".to_string()),
        }
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum BlockCommand {
    List(FileArgs),
    Get(BlockGetArgs),
    Ensure(BlockEnsureArgs),
    Replace(BlockReplaceArgs),
    Lock(BlockGetMutateArgs),
    Unlock(BlockGetMutateArgs),
}

#[derive(Debug, Args)]
pub(crate) struct BlockGetArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) id: String,
}

#[derive(Debug, Args)]
pub(crate) struct BlockGetMutateArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) id: String,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct BlockEnsureArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) parent_section: String,
    #[arg(long)]
    pub(crate) id: String,
    #[arg(long, conflicts_with = "text")]
    pub(crate) body_from_file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) text: Option<String>,
    #[arg(long, default_value = "end")]
    pub(crate) position: String,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct BlockReplaceArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) id: String,
    #[arg(long)]
    pub(crate) body_from_file: PathBuf,
    #[arg(long, default_value = "fail")]
    pub(crate) on_modified: OnModified,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum OnModified {
    Fail,
    Force,
    ThreeWay,
}

impl std::str::FromStr for OnModified {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fail" => Ok(Self::Fail),
            "force" => Ok(Self::Force),
            "three-way" => Ok(Self::ThreeWay),
            _ => Err("expected fail, force, or three-way".to_string()),
        }
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum FrontmatterCommand {
    Get(FrontmatterGetArgs),
    Set(FrontmatterSetArgs),
    Delete(FrontmatterDeleteArgs),
}

#[derive(Debug, Args)]
pub(crate) struct FrontmatterGetArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) key: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct FrontmatterSetArgs {
    pub(crate) file: PathBuf,
    pub(crate) key: String,
    pub(crate) value: String,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct FrontmatterDeleteArgs {
    pub(crate) file: PathBuf,
    pub(crate) key: String,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct LintArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) rules: Option<String>,
    #[arg(long)]
    pub(crate) fix: Option<String>,
    #[command(flatten)]
    pub(crate) mutate: MutateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct ValidateArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) schema: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct FileArgs {
    pub(crate) file: PathBuf,
}

#[derive(Debug, Args, Clone)]
pub(crate) struct MutateArgs {
    /// Atomically write the transformed document in place.
    #[arg(long)]
    pub(crate) write: bool,
    /// Emit plan, document, or json. Mutations default to plan output.
    #[arg(long, value_enum, default_value = "plan")]
    pub(crate) emit: EmitMode,
    /// Refuse to mutate if the input hash does not match.
    #[arg(long)]
    pub(crate) preimage_hash: Option<String>,
}

#[derive(Debug, Clone, ValueEnum, PartialEq, Eq)]
pub(crate) enum EmitMode {
    Plan,
    Document,
    Json,
}

pub(crate) fn run(cli: Cli) -> Result<Outcome, MdliError> {
    match cli.command {
        Commands::Id(cmd) => run_id(cmd),
        Commands::Section(cmd) => run_section(cmd),
        Commands::Table(cmd) => run_table(cmd),
        Commands::Block(cmd) => run_block(cmd),
        Commands::Frontmatter(cmd) => run_frontmatter(cmd),
        Commands::Lint(args) => run_lint(args),
        Commands::Validate(args) => run_validate(args),
        Commands::Inspect(args) => run_inspect(args),
        Commands::Tree(args) => run_tree(args),
        Commands::Context(args) => run_context(args),
        Commands::Template(cmd) => run_template(cmd),
        Commands::Recipe(cmd) => run_recipe(cmd),
        Commands::Apply(args) => run_apply(args),
        Commands::Build(args) => run_build(args),
        Commands::Plan(args) => run_plan(args),
        Commands::ApplyPlan(args) => run_apply_plan(args),
        Commands::Patch(args) => run_patch(args),
        Commands::Diff(args) => run_diff(args),
    }
}
