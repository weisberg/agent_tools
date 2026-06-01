mod clean;
mod excel;
mod excel_edit;
mod history;
mod lint;
mod lists;
mod model;
mod pb;
mod render;
mod rtf;
mod store;
mod templatize;

use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

use clean::{CleanOptions, TargetApp};
use model::{PbType, TableInput, TemplateMeta};
use pb::PbError;
use render::Renderer;
use store::{ListFilter, SaveContent, Store};
use templatize::TemplatizeResult;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Default, serde::Deserialize)]
struct Config {
    #[serde(default)]
    defaults: ConfigDefaults,
    #[serde(default)]
    clean: ConfigClean,
    #[serde(default)]
    templatize: ConfigTemplatize,
    #[serde(default)]
    agent: ConfigAgent,
}

#[derive(Debug, serde::Deserialize)]
struct ConfigDefaults {
    #[serde(default = "default_font")]
    font: String,
    #[serde(default = "default_font_size")]
    font_size_pt: f32,
    #[serde(default = "default_plain_text_strategy")]
    plain_text_strategy: String,
}

fn default_font() -> String {
    "Calibri".to_string()
}
fn default_font_size() -> f32 {
    11.0
}
fn default_plain_text_strategy() -> String {
    "tab-delimited".to_string()
}

impl Default for ConfigDefaults {
    fn default() -> Self {
        Self {
            font: default_font(),
            font_size_pt: default_font_size(),
            plain_text_strategy: default_plain_text_strategy(),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct ConfigClean {
    #[serde(default)]
    keep_classes: bool,
    #[serde(default = "default_target_app")]
    target_app: String,
}

fn default_target_app() -> String {
    "generic".to_string()
}

impl Default for ConfigClean {
    fn default() -> Self {
        Self {
            keep_classes: false,
            target_app: default_target_app(),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct ConfigTemplatize {
    #[serde(default = "default_strategy")]
    default_strategy: String,
}

fn default_strategy() -> String {
    "heuristic".to_string()
}

impl Default for ConfigTemplatize {
    fn default() -> Self {
        Self {
            default_strategy: default_strategy(),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct ConfigAgent {
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default = "default_agent_timeout")]
    timeout_secs: u64,
}

fn default_agent_timeout() -> u64 {
    30
}

impl Default for ConfigAgent {
    fn default() -> Self {
        Self {
            command: None,
            args: Vec::new(),
            timeout_secs: default_agent_timeout(),
        }
    }
}

fn load_config() -> Config {
    let config_path = config_file_path();
    if config_path.exists() {
        if let Ok(s) = std::fs::read_to_string(&config_path) {
            if let Ok(c) = toml::from_str::<Config>(&s) {
                return c;
            }
        }
    }
    Config::default()
}

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "clipli",
    version,
    about = "Clipboard intelligence CLI — template-driven pasteboard for agents and power users",
    after_help = "Examples:\n  clipli inspect --json\n  clipli capture --name quarterly_report --templatize\n  clipli paste quarterly_report -D '{\"quarter\":\"Q2\"}'\n  clipli preview quarterly_report -D '{\"quarter\":\"Q2\"}' --open\n  printf 'Name,Revenue\\nAlice,1200\\n' | clipli excel --preset finance --copy-as svg\n  clipli list-build --item 'Launch > QA' --item 'Launch > Docs'\n  clipli history prune --keep-latest 200 --dry-run --json"
)]
struct Cli {
    /// Increase verbosity (-v info, -vv debug, -vvv trace)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Show all types currently on the clipboard
    Inspect {
        #[arg(long)]
        json: bool,
    },
    /// Read clipboard content and output to stdout
    Read {
        #[arg(long, short = 't', default_value = "html")]
        r#type: String,
        #[arg(long, short = 'c')]
        clean: bool,
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
    /// Write content from stdin or file to the clipboard
    Write {
        #[arg(long, short = 't', default_value = "html")]
        r#type: String,
        #[arg(long, short = 'i')]
        input: Option<PathBuf>,
        #[arg(long, default_value = "true")]
        with_plain: bool,
    },
    /// Capture clipboard content as a named template
    Capture {
        #[arg(long, short = 'n')]
        name: String,
        #[arg(long, short = 't')]
        templatize: bool,
        #[arg(long)]
        strategy: Option<String>,
        #[arg(long, short = 'd')]
        description: Option<String>,
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        #[arg(long, short = 'f')]
        force: bool,
        #[arg(long)]
        raw: bool,
        #[arg(long)]
        keep_classes: bool,
        /// External command to invoke for agent strategy
        #[arg(long)]
        agent_command: Option<String>,
        /// Timeout in seconds for agent response
        #[arg(long, default_value = "30")]
        agent_timeout: u64,
        /// Preview cleaned/templatized HTML in browser before saving
        #[arg(long)]
        preview: bool,
        #[arg(long)]
        json: bool,
    },
    /// Render a template with data and write to clipboard
    Paste {
        name: Option<String>,
        #[arg(long = "data", short = 'D')]
        data: Option<String>,
        #[arg(long)]
        data_file: Option<PathBuf>,
        #[arg(long)]
        stdin: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, default_value = "auto")]
        plain_text: String,
        #[arg(long)]
        open: bool,
        #[arg(long)]
        from_table: bool,
        #[arg(long, short = 't', default_value = "table_default")]
        template: String,
        #[arg(long)]
        json: bool,
    },
    /// Render or save an explicit HTML preview without touching the clipboard
    Preview {
        /// Template name to render; omit when previewing stdin or --input HTML
        name: Option<String>,
        /// HTML input file to preview instead of a stored template
        #[arg(long, short = 'i')]
        input: Option<PathBuf>,
        /// Inline JSON data for template rendering
        #[arg(long = "data", short = 'D')]
        data: Option<String>,
        /// JSON data file for template rendering
        #[arg(long)]
        data_file: Option<PathBuf>,
        /// Open the generated preview file in the default browser
        #[arg(long)]
        open: bool,
        /// Write preview HTML to this path instead of the preview cache
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// List all saved templates
    List {
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        json: bool,
        /// Show variable details for each template
        #[arg(long, short = 'd')]
        detail: bool,
    },
    /// Show details of a specific template
    Show {
        name: String,
        #[arg(long)]
        html: bool,
        #[arg(long)]
        schema: bool,
        #[arg(long)]
        meta: bool,
        #[arg(long)]
        open: bool,
        /// Show a specific version instead of the live template
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Open a template in $EDITOR for manual editing
    Edit {
        name: String,
        #[arg(long)]
        auto_schema: bool,
    },
    /// Delete a template
    Delete {
        name: String,
        #[arg(long, short = 'f')]
        force: bool,
        /// Delete live template but preserve version history
        #[arg(long)]
        keep_versions: bool,
        #[arg(long)]
        json: bool,
    },
    /// List version history for a template
    Versions {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Restore a template from a previous version
    Restore {
        name: String,
        /// Version ID to restore (from `clipli versions`)
        #[arg(long)]
        version: String,
    },
    /// Lint a template for variable mismatches and syntax issues
    Lint {
        name: String,
        /// Treat warnings as errors
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        json: bool,
    },
    /// Search templates by name, description, tags, or content
    Search {
        query: String,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Export a template as a .clipli bundle
    Export {
        name: String,
        /// Output file path (default: ./<name>.clipli)
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
    },
    /// Import a template from a .clipli bundle
    Import {
        /// Path to .clipli bundle file
        file: PathBuf,
        #[arg(long, short = 'f')]
        force: bool,
        /// Override the template name from the bundle
        #[arg(long)]
        name: Option<String>,
    },
    /// Generate an Excel-style table from CSV/JSON stdin or a file as HTML, SVG, or PNG
    Excel {
        /// CSV/JSON file path. Omit to read one-shot input from stdin.
        file: Option<PathBuf>,
        /// Input format: csv, json, or auto
        #[arg(long = "input-format", default_value = "auto", value_parser = ["csv", "json", "auto"])]
        input_format: String,
        /// Named formatting preset: default, finance, executive, minimal, or status
        #[arg(long, value_parser = ["default", "finance", "executive", "minimal", "status"])]
        preset: Option<String>,
        /// Table style: "table" (banded rows) or "plain" (thick borders)
        #[arg(long)]
        style: Option<String>,
        /// Header background color
        #[arg(long)]
        header_bg: Option<String>,
        /// Header text color
        #[arg(long)]
        header_fg: Option<String>,
        /// Banded row background color (table style only)
        #[arg(long)]
        band_bg: Option<String>,
        /// Font family
        #[arg(long)]
        font: Option<String>,
        /// Font size in pt
        #[arg(long)]
        font_size: Option<String>,
        /// Column format: NAME:FORMAT[:ALIGN] (repeatable)
        #[arg(long = "col", value_name = "NAME:FMT[:ALIGN]")]
        col_specs: Vec<String>,
        /// Column alignment without format: NAME:ALIGN (repeatable)
        #[arg(long = "align", value_name = "NAME:ALIGN")]
        align_specs: Vec<String>,
        /// Make a column bold (repeatable)
        #[arg(long = "bold")]
        bold_cols: Vec<String>,
        /// Make a column italic (repeatable)
        #[arg(long = "italic")]
        italic_cols: Vec<String>,
        /// Enable word wrap for a column (repeatable)
        #[arg(long = "wrap")]
        wrap_cols: Vec<String>,
        /// Column text color: NAME:HEX (repeatable)
        #[arg(long = "fg-color", value_name = "NAME:HEX")]
        fg_colors: Vec<String>,
        /// Column background color: NAME:HEX (repeatable)
        #[arg(long = "bg-color", value_name = "NAME:HEX")]
        bg_colors: Vec<String>,
        /// Conditional color: COLUMN:OP:VALUE:BG_HEX:FG_HEX (repeatable).
        /// Ops: >=, <=, >, <, ==, !=, contains, empty, not_empty
        #[arg(long = "color-if", value_name = "SPEC")]
        color_rules: Vec<String>,
        /// Hyperlink pattern: NAME:URL_PATTERN with {} placeholder (repeatable)
        #[arg(long = "link", value_name = "NAME:URL")]
        links: Vec<String>,
        /// Clipboard artifact to generate: html (editable table), svg, or png
        #[arg(long, default_value = "html")]
        copy_as: String,
        /// Write dry-run output to a file; required for PNG dry-run
        #[arg(long, short = 'o')]
        out_file: Option<PathBuf>,
        /// PNG scale factor (e.g. 2.5 = 250% size). Only applies when --copy-as png. Default 1.0
        #[arg(long, default_value = "1.0")]
        png_scale: f32,
        /// Merged title row above the header
        #[arg(long)]
        title: Option<String>,
        /// Add a total row (auto-sums numeric columns)
        #[arg(long)]
        total_row: bool,
        /// Use SUM formulas in total row instead of pre-computed values
        #[arg(long)]
        total_formula: bool,
        /// Per-cell formula: COL:ROW:FORMULA (row is 0-based data row index, repeatable)
        #[arg(long = "formula", value_name = "COL:ROW:EXPR")]
        formulas: Vec<String>,
        /// Row height in pixels
        #[arg(long)]
        row_height: Option<u32>,
        /// Header row height in pixels
        #[arg(long)]
        header_height: Option<u32>,
        /// Select and order columns: COL1,COL2,... (comma-separated)
        #[arg(long, value_delimiter = ',')]
        columns: Option<Vec<String>>,
        /// Hide a column (repeatable)
        #[arg(long = "hide")]
        hidden_cols: Vec<String>,
        /// Rename a column header: OLD:NEW (repeatable)
        #[arg(long = "rename", value_name = "OLD:NEW")]
        renames: Vec<String>,
        /// Column font size override: NAME:SIZE (repeatable)
        #[arg(long = "col-font-size", value_name = "NAME:SIZE")]
        col_font_sizes: Vec<String>,
        /// Print or write the generated artifact instead of writing to clipboard
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Edit cells in the clipboard's Excel HTML by A1 reference
    #[command(name = "excel-edit")]
    ExcelEdit {
        /// Set cell value: CELL:VALUE (e.g. A2:Hello)
        #[arg(long = "set", value_name = "CELL:VALUE")]
        set_values: Vec<String>,
        /// Set cell background: CELL:HEX (e.g. C3:#A0D771)
        #[arg(long = "set-bg", value_name = "CELL:HEX")]
        set_bgs: Vec<String>,
        /// Set cell text color: CELL:HEX
        #[arg(long = "set-fg", value_name = "CELL:HEX")]
        set_fgs: Vec<String>,
        /// Set cell number format: CELL:FORMAT
        #[arg(long = "set-format", value_name = "CELL:FMT")]
        set_formats: Vec<String>,
        /// Set cell formula: CELL:FORMULA (e.g. E6:=SUM(E2:E5))
        #[arg(long = "set-formula", value_name = "CELL:EXPR")]
        set_formulas: Vec<String>,
        /// Set cell alignment: CELL:ALIGN
        #[arg(long = "set-align", value_name = "CELL:ALIGN")]
        set_aligns: Vec<String>,
        /// Make cell bold: CELL (e.g. A2)
        #[arg(long = "set-bold", value_name = "CELL")]
        set_bolds: Vec<String>,
        /// Make cell italic: CELL
        #[arg(long = "set-italic", value_name = "CELL")]
        set_italics: Vec<String>,
        /// Enable word wrap on cell: CELL
        #[arg(long = "set-wrap", value_name = "CELL")]
        set_wraps: Vec<String>,
        /// Print modified HTML to stdout instead of writing to clipboard
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Build a nested list and copy it as HTML or Markdown
    #[command(name = "list-build")]
    ListBuild {
        /// JSON, Markdown, HTML, or indented-lines input. Omit to use --item flags or stdin.
        file: Option<PathBuf>,
        /// Input format: auto, json, markdown, lines, or html
        #[arg(long = "input-format", default_value = "auto", value_parser = ["auto", "json", "markdown", "lines", "html"])]
        input_format: String,
        /// Add an item path, e.g. "Launch > [x] QA" (repeatable)
        #[arg(long = "item", value_name = "PATH")]
        item_specs: Vec<String>,
        /// List kind: unordered or ordered
        #[arg(long, value_parser = ["unordered", "ordered"])]
        kind: Option<String>,
        /// Shortcut for --kind ordered
        #[arg(long)]
        ordered: bool,
        /// Optional heading rendered above the list
        #[arg(long)]
        title: Option<String>,
        /// Clipboard artifact to generate: html or markdown
        #[arg(long, default_value = "html", value_parser = ["html", "markdown"])]
        copy_as: String,
        /// Write dry-run output to a file
        #[arg(long, short = 'o')]
        out_file: Option<PathBuf>,
        /// Font family for HTML output
        #[arg(long)]
        font: Option<String>,
        /// Font size in pt for HTML output
        #[arg(long)]
        font_size: Option<String>,
        /// CSS class for the HTML wrapper
        #[arg(long = "class")]
        class_name: Option<String>,
        /// Render compact list spacing in HTML
        #[arg(long)]
        tight: bool,
        /// Sort items alphabetically at every level before rendering
        #[arg(long)]
        sort: bool,
        /// Remove duplicate sibling items before rendering
        #[arg(long)]
        dedupe: bool,
        /// Print or write the generated list instead of writing to clipboard
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Edit a nested list from clipboard, stdin, or file, then copy it as HTML or Markdown
    #[command(name = "list-edit")]
    ListEdit {
        /// Existing list input. Omit to read stdin, then fall back to clipboard.
        file: Option<PathBuf>,
        /// Input format: auto, json, markdown, lines, or html
        #[arg(long = "input-format", default_value = "auto", value_parser = ["auto", "json", "markdown", "lines", "html"])]
        input_format: String,
        /// List kind: unordered or ordered
        #[arg(long, value_parser = ["unordered", "ordered"])]
        kind: Option<String>,
        /// Shortcut for --kind ordered
        #[arg(long)]
        ordered: bool,
        /// Replace or set the heading rendered above the list
        #[arg(long)]
        title: Option<String>,
        /// Set item text: PATH:TEXT (e.g. 1.2:Updated)
        #[arg(long = "set", value_name = "PATH:TEXT")]
        set_values: Vec<String>,
        /// Append item to root or parent path: [PATH:]TEXT
        #[arg(long = "append", value_name = "[PATH:]TEXT")]
        append_items: Vec<String>,
        /// Insert item before path: PATH:TEXT
        #[arg(long = "insert-before", value_name = "PATH:TEXT")]
        insert_before: Vec<String>,
        /// Insert item after path: PATH:TEXT
        #[arg(long = "insert-after", value_name = "PATH:TEXT")]
        insert_after: Vec<String>,
        /// Remove item by path (repeatable)
        #[arg(long = "remove", value_name = "PATH")]
        remove_paths: Vec<String>,
        /// Mark item checked by path (repeatable)
        #[arg(long = "check", value_name = "PATH")]
        check_paths: Vec<String>,
        /// Mark item unchecked by path (repeatable)
        #[arg(long = "uncheck", value_name = "PATH")]
        uncheck_paths: Vec<String>,
        /// Toggle item checked state by path (repeatable)
        #[arg(long = "toggle", value_name = "PATH")]
        toggle_paths: Vec<String>,
        /// Move item under its previous sibling (repeatable)
        #[arg(long = "indent", value_name = "PATH")]
        indent_paths: Vec<String>,
        /// Move item one level up after its parent (repeatable)
        #[arg(long = "outdent", value_name = "PATH")]
        outdent_paths: Vec<String>,
        /// Sort children at PATH; use root for the top level (repeatable)
        #[arg(long = "sort", value_name = "PATH")]
        sort_paths: Vec<String>,
        /// Remove duplicate sibling items at every level
        #[arg(long)]
        dedupe: bool,
        /// Clipboard artifact to generate: html or markdown
        #[arg(long, default_value = "html", value_parser = ["html", "markdown"])]
        copy_as: String,
        /// Write dry-run output to a file
        #[arg(long, short = 'o')]
        out_file: Option<PathBuf>,
        /// Font family for HTML output
        #[arg(long)]
        font: Option<String>,
        /// Font size in pt for HTML output
        #[arg(long)]
        font_size: Option<String>,
        /// CSS class for the HTML wrapper
        #[arg(long = "class")]
        class_name: Option<String>,
        /// Render compact list spacing in HTML
        #[arg(long)]
        tight: bool,
        /// Print or write the modified list instead of writing to clipboard
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Render a template with multiple data rows to files or stdout
    Render {
        /// Template name
        name: String,
        /// JSON file containing an array of data objects (or newline-delimited JSON)
        #[arg(long)]
        data_file: PathBuf,
        /// Output directory for rendered files (001.html, 002.html, ...)
        #[arg(long, short = 'o')]
        output_dir: Option<PathBuf>,
        /// Output format: "html" or "plain"
        #[arg(long, default_value = "html")]
        format: String,
        #[arg(long)]
        json: bool,
    },
    /// Convert between formats (stdin/stdout)
    Convert {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long, short = 'i')]
        input: Option<PathBuf>,
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        #[arg(long = "data", short = 'D')]
        data: Option<String>,
        #[arg(long, default_value = "heuristic")]
        strategy: String,
        #[arg(long)]
        json: bool,
    },
    /// Check local environment, config, and clipboard readiness
    Doctor {
        #[arg(long)]
        json: bool,
        /// Do not touch the macOS pasteboard; useful for CI
        #[arg(long)]
        skip_clipboard: bool,
    },
    /// Generate shell completions
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Searchable, privacy-aware clipboard history
    History {
        #[command(subcommand)]
        command: HistoryCommand,
    },
    /// Capture clipboard changes into history
    Watch {
        /// Capture one current clipboard snapshot and exit
        #[arg(long)]
        once: bool,
        /// Maximum entries to record before exiting
        #[arg(long)]
        max_items: Option<usize>,
        /// Polling interval in milliseconds
        #[arg(long, default_value = "1000")]
        interval_ms: u64,
        /// Prune history after each recorded item, keeping the newest N entries
        #[arg(long)]
        max_history: Option<usize>,
        /// Sensitive payload handling: skip, redact, or allow
        #[arg(long, default_value = "skip", value_parser = ["skip", "redact", "allow"])]
        sensitive: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum HistoryCommand {
    /// Record clipboard content or test input into history
    Record {
        /// Pasteboard type to record
        #[arg(long, short = 't', default_value = "plain")]
        r#type: String,
        /// Read payload from file instead of the clipboard
        #[arg(long, short = 'i')]
        input: Option<PathBuf>,
        /// Source app label for file-based records
        #[arg(long)]
        source_app: Option<String>,
        /// Sensitive payload handling: skip, redact, or allow
        #[arg(long, default_value = "skip", value_parser = ["skip", "redact", "allow"])]
        sensitive: String,
        #[arg(long)]
        json: bool,
    },
    /// List recorded history entries
    List {
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Filter by source app substring
        #[arg(long)]
        source_app: Option<String>,
        /// Filter by pasteboard type
        #[arg(long = "type")]
        r#type: Option<String>,
        /// Filter entries captured at or after this date/time
        #[arg(long)]
        from: Option<String>,
        /// Filter entries captured at or before this date/time
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Search metadata and text payloads
    Search {
        query: String,
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Filter by source app substring
        #[arg(long)]
        source_app: Option<String>,
        /// Filter by pasteboard type
        #[arg(long = "type")]
        r#type: Option<String>,
        /// Filter entries captured at or after this date/time
        #[arg(long)]
        from: Option<String>,
        /// Filter entries captured at or before this date/time
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show one history entry
    Show {
        id: String,
        /// Include text payload preview when available
        #[arg(long)]
        content: bool,
        #[arg(long)]
        json: bool,
    },
    /// Restore one history entry to the clipboard or dry-run output
    Restore {
        id: String,
        /// Print or write payload instead of mutating the clipboard
        #[arg(long)]
        dry_run: bool,
        /// Write dry-run payload to a file
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Remove old history entries and payloads
    Prune {
        /// Keep the newest N entries matching the filters
        #[arg(long)]
        keep_latest: Option<usize>,
        /// Prune entries captured before this date/time
        #[arg(long)]
        before: Option<String>,
        /// Filter by source app substring
        #[arg(long)]
        source_app: Option<String>,
        /// Filter by pasteboard type
        #[arg(long = "type")]
        r#type: Option<String>,
        /// Filter entries captured at or after this date/time
        #[arg(long)]
        from: Option<String>,
        /// Filter entries captured at or before this date/time
        #[arg(long)]
        to: Option<String>,
        /// Report what would be removed without deleting anything
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    // Initialize tracing subscriber
    let log_level = match cli.verbose {
        0 => tracing::Level::ERROR,
        1 => tracing::Level::INFO,
        2 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    if std::env::var("RUST_LOG").is_ok() {
        // Honor RUST_LOG if set
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(std::io::stderr)
            .init();
    } else if cli.verbose > 0 {
        tracing_subscriber::fmt()
            .with_max_level(log_level)
            .with_writer(std::io::stderr)
            .init();
    }

    let config = load_config();

    // Detect --json mode before dispatching (so errors can be reported as JSON)
    let json_mode = matches!(
        &cli.command,
        Commands::Inspect { json: true, .. }
            | Commands::Capture { json: true, .. }
            | Commands::Preview { json: true, .. }
            | Commands::List { json: true, .. }
            | Commands::Paste { json: true, .. }
            | Commands::Show { json: true, .. }
            | Commands::Delete { json: true, .. }
            | Commands::Versions { json: true, .. }
            | Commands::Lint { json: true, .. }
            | Commands::Search { json: true, .. }
            | Commands::Excel { json: true, .. }
            | Commands::ExcelEdit { json: true, .. }
            | Commands::ListBuild { json: true, .. }
            | Commands::ListEdit { json: true, .. }
            | Commands::Render { json: true, .. }
            | Commands::Convert { json: true, .. }
            | Commands::Doctor { json: true, .. }
            | Commands::Watch { json: true, .. }
            | Commands::History {
                command: HistoryCommand::Record { json: true, .. }
                    | HistoryCommand::List { json: true, .. }
                    | HistoryCommand::Search { json: true, .. }
                    | HistoryCommand::Show { json: true, .. }
                    | HistoryCommand::Restore { json: true, .. }
                    | HistoryCommand::Prune { json: true, .. },
            }
    );

    if let Err(e) = run(cli.command, &config) {
        if json_mode {
            let code = try_error_code(&*e);
            print_json_error(&e.to_string(), code);
        } else {
            eprintln!("error: {e}");
        }
        std::process::exit(1);
    }
}

fn run(cmd: Commands, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Commands::Inspect { json } => cmd_inspect(json),
        Commands::Read {
            r#type,
            clean,
            output,
        } => cmd_read(r#type, clean, output, config),
        Commands::Write {
            r#type,
            input,
            with_plain,
        } => cmd_write(r#type, input, with_plain),
        Commands::Capture {
            name,
            templatize,
            strategy,
            description,
            tags,
            force,
            raw,
            keep_classes,
            agent_command,
            agent_timeout,
            preview,
            json,
        } => cmd_capture(
            name,
            templatize,
            strategy,
            description,
            tags,
            force,
            raw,
            keep_classes,
            agent_command,
            agent_timeout,
            preview,
            json,
            config,
        ),
        Commands::Paste {
            name,
            data,
            data_file,
            stdin,
            dry_run,
            plain_text,
            open,
            from_table,
            template,
            json,
        } => cmd_paste(
            name, data, data_file, stdin, dry_run, plain_text, open, from_table, template, json,
            config,
        ),
        Commands::Preview {
            name,
            input,
            data,
            data_file,
            open,
            output,
            json,
        } => cmd_preview(name, input, data, data_file, open, output, json),
        Commands::List { tag, json, detail } => cmd_list(tag, json, detail),
        Commands::Show {
            name,
            html,
            schema,
            meta,
            open,
            version,
            json,
        } => cmd_show(name, html, schema, meta, open, version, json),
        Commands::Edit { name, auto_schema } => cmd_edit(name, auto_schema),
        Commands::Delete {
            name,
            force,
            keep_versions,
            json,
        } => cmd_delete(name, force, keep_versions, json),
        Commands::Versions { name, json } => cmd_versions(name, json),
        Commands::Restore { name, version } => cmd_restore(name, version),
        Commands::Lint { name, strict, json } => cmd_lint(name, strict, json),
        Commands::Search { query, tag, json } => cmd_search(query, tag, json),
        Commands::Export { name, output } => cmd_export(name, output),
        Commands::Import { file, force, name } => cmd_import(file, force, name),
        Commands::Excel {
            file,
            input_format,
            preset,
            style,
            header_bg,
            header_fg,
            band_bg,
            font,
            font_size,
            col_specs,
            align_specs,
            bold_cols,
            italic_cols,
            wrap_cols,
            fg_colors,
            bg_colors,
            color_rules,
            links,
            copy_as,
            out_file,
            png_scale,
            title,
            total_row,
            total_formula,
            formulas,
            row_height,
            header_height,
            columns,
            hidden_cols,
            renames,
            col_font_sizes,
            dry_run,
            json,
        } => cmd_excel(
            file,
            input_format,
            preset,
            style,
            header_bg,
            header_fg,
            band_bg,
            font,
            font_size,
            col_specs,
            align_specs,
            bold_cols,
            italic_cols,
            wrap_cols,
            fg_colors,
            bg_colors,
            color_rules,
            links,
            copy_as,
            out_file,
            png_scale,
            title,
            total_row,
            total_formula,
            formulas,
            row_height,
            header_height,
            columns,
            hidden_cols,
            renames,
            col_font_sizes,
            dry_run,
            json,
            config,
        ),
        Commands::ExcelEdit {
            set_values,
            set_bgs,
            set_fgs,
            set_formats,
            set_formulas,
            set_aligns,
            set_bolds,
            set_italics,
            set_wraps,
            dry_run,
            json,
        } => cmd_excel_edit(
            set_values,
            set_bgs,
            set_fgs,
            set_formats,
            set_formulas,
            set_aligns,
            set_bolds,
            set_italics,
            set_wraps,
            dry_run,
            json,
        ),
        Commands::ListBuild {
            file,
            input_format,
            item_specs,
            kind,
            ordered,
            title,
            copy_as,
            out_file,
            font,
            font_size,
            class_name,
            tight,
            sort,
            dedupe,
            dry_run,
            json,
        } => cmd_list_build(
            file,
            input_format,
            item_specs,
            kind,
            ordered,
            title,
            copy_as,
            out_file,
            font,
            font_size,
            class_name,
            tight,
            sort,
            dedupe,
            dry_run,
            json,
            config,
        ),
        Commands::ListEdit {
            file,
            input_format,
            kind,
            ordered,
            title,
            set_values,
            append_items,
            insert_before,
            insert_after,
            remove_paths,
            check_paths,
            uncheck_paths,
            toggle_paths,
            indent_paths,
            outdent_paths,
            sort_paths,
            dedupe,
            copy_as,
            out_file,
            font,
            font_size,
            class_name,
            tight,
            dry_run,
            json,
        } => cmd_list_edit(
            file,
            input_format,
            kind,
            ordered,
            title,
            set_values,
            append_items,
            insert_before,
            insert_after,
            remove_paths,
            check_paths,
            uncheck_paths,
            toggle_paths,
            indent_paths,
            outdent_paths,
            sort_paths,
            dedupe,
            copy_as,
            out_file,
            font,
            font_size,
            class_name,
            tight,
            dry_run,
            json,
            config,
        ),
        Commands::Render {
            name,
            data_file,
            output_dir,
            format,
            json,
        } => cmd_render(name, data_file, output_dir, format, json),
        Commands::Convert {
            from,
            to,
            input,
            output,
            data,
            strategy,
            json,
        } => cmd_convert(from, to, input, output, data, strategy, json, config),
        Commands::Doctor {
            json,
            skip_clipboard,
        } => cmd_doctor(json, skip_clipboard, config),
        Commands::Completions { shell } => cmd_completions(shell),
        Commands::History { command } => cmd_history(command),
        Commands::Watch {
            once,
            max_items,
            interval_ms,
            max_history,
            sensitive,
            json,
        } => cmd_watch(once, max_items, interval_ms, max_history, sensitive, json),
    }
}

// ---------------------------------------------------------------------------
// Command implementations
// ---------------------------------------------------------------------------

fn cmd_inspect(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    match pb::read_all() {
        Ok(snapshot) => {
            if json {
                let types: Vec<serde_json::Value> = snapshot
                    .types
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "uti": e.uti,
                            "size_bytes": e.size_bytes,
                        })
                    })
                    .collect();
                let out = serde_json::json!({
                    "types": types,
                    "source_app": snapshot.source_app,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("Pasteboard contents ({} types):", snapshot.types.len());
                for entry in &snapshot.types {
                    println!(
                        "  {:<35} {:>10} bytes",
                        entry.uti,
                        format_with_commas(entry.size_bytes as u64)
                    );
                }
                if let Some(app) = &snapshot.source_app {
                    println!("Source app: {}", app);
                }
            }
        }
        Err(PbError::Empty) => {
            if json {
                println!("{}", serde_json::json!({"types": [], "source_app": null}));
            } else {
                println!("Pasteboard is empty");
            }
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

fn cmd_read(
    type_: String,
    do_clean: bool,
    output: Option<PathBuf>,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let pb_type = parse_pb_type(&type_)?;

    // Binary types require --output
    let is_binary = matches!(pb_type, PbType::Png | PbType::Tiff | PbType::Pdf);
    if is_binary && output.is_none() {
        return Err(format!("binary type '{}' requires --output <file>", type_).into());
    }

    let data = pb::read_type(pb_type)?;

    if is_binary {
        let path = output.unwrap();
        std::fs::write(&path, &data)?;
        eprintln!("Wrote {} bytes to {}", data.len(), path.display());
        return Ok(());
    }

    // Text path
    let text = String::from_utf8(data)?;
    let content = if do_clean && pb_type == PbType::Html {
        let opts = CleanOptions {
            keep_classes: config.clean.keep_classes,
            target_app: parse_target_app(&config.clean.target_app),
        };
        clean::clean(&text, &opts)?
    } else {
        text
    };

    match output {
        Some(path) => std::fs::write(&path, content.as_bytes())?,
        None => print!("{}", content),
    }
    Ok(())
}

fn cmd_write(
    type_: String,
    input: Option<PathBuf>,
    with_plain: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = match input {
        Some(path) => std::fs::read_to_string(&path)?,
        None => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };

    let pb_type = parse_pb_type(&type_)?;

    match pb_type {
        PbType::Html => {
            let plain = if with_plain {
                Some(render::html_to_plain_text(&content))
            } else {
                None
            };
            pb::write_html(&content, plain.as_deref())?;
        }
        _ => {
            pb::write(&[(pb_type, content.as_bytes())])?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_capture(
    name: String,
    do_templatize: bool,
    strategy: Option<String>,
    description: Option<String>,
    tags: Vec<String>,
    force: bool,
    raw: bool,
    keep_classes: bool,
    agent_command: Option<String>,
    agent_timeout: u64,
    preview: bool,
    json: bool,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let strategy = strategy.unwrap_or_else(|| config.templatize.default_strategy.clone());
    if !store::validate_name(&name) {
        return Err(format!(
            "invalid template name '{}': use only letters, digits, underscores, and hyphens",
            name
        )
        .into());
    }

    let s = Store::new()?;

    // Read from pasteboard — prefer HTML, fall back to RTF, then plain text
    let (raw_bytes, source_pb_type) = {
        match pb::read_type(PbType::Html) {
            Ok(data) => (data, PbType::Html),
            Err(_) => match pb::read_type(PbType::Rtf) {
                Ok(data) => (data, PbType::Rtf),
                Err(_) => {
                    let data = pb::read_type(PbType::PlainText)?;
                    (data, PbType::PlainText)
                }
            },
        }
    };

    let snapshot = pb::read_all().ok();
    let source_app = snapshot.as_ref().and_then(|s| s.source_app.clone());
    let source_pb_types: Vec<String> = snapshot
        .as_ref()
        .map(|s| s.types.iter().map(|e| e.uti.clone()).collect())
        .unwrap_or_else(|| vec![source_pb_type.uti().to_string()]);

    let raw_html = if source_pb_type == PbType::Rtf {
        match rtf::rtf_to_html(&raw_bytes) {
            Ok(html) => html,
            Err(_) => String::from_utf8_lossy(&raw_bytes).into_owned(),
        }
    } else {
        String::from_utf8_lossy(&raw_bytes).into_owned()
    };

    // Optionally clean
    let cleaned_html = if raw {
        raw_html.clone()
    } else {
        let target_app_str = config.clean.target_app.as_str();
        let opts = CleanOptions {
            keep_classes: keep_classes || config.clean.keep_classes,
            target_app: parse_target_app(target_app_str),
        };
        clean::clean(&raw_html, &opts)?
    };

    // Determine the effective strategy
    let eff_strategy = if do_templatize {
        strategy.as_str()
    } else {
        "manual"
    };
    tracing::info!(strategy = %eff_strategy, "capture: strategy selected");

    let TemplatizeResult {
        template_html,
        variables,
    } = match eff_strategy {
        "agent" => {
            let agent_cfg = templatize::AgentConfig {
                command: agent_command.or(config.agent.command.clone()),
                args: config.agent.args.clone(),
                timeout_secs: agent_timeout,
            };
            templatize::agent(&cleaned_html, source_app.as_deref(), &agent_cfg)?
        }
        "heuristic" => templatize::heuristic(&cleaned_html),
        _ => templatize::manual(&cleaned_html),
    };

    tracing::info!(
        variables = variables.len(),
        "capture: templatization complete"
    );

    let is_templatized = do_templatize && eff_strategy != "manual";

    let now = chrono::Utc::now();
    let meta = TemplateMeta {
        name: name.clone(),
        description,
        created_at: now,
        updated_at: now,
        source_app,
        source_pb_types,
        templatized: is_templatized,
        variables: variables.clone(),
        tags,
    };

    let schema = if variables.is_empty() {
        None
    } else {
        Some(variables.clone())
    };

    if preview {
        let preview_path = write_preview_file("capture", &template_html, None)?;
        open_in_browser(&preview_path)?;
    }

    let content = SaveContent {
        template_html,
        is_templatized,
        meta: meta.clone(),
        schema,
        original_html: Some(cleaned_html),
        raw_html: if raw { None } else { Some(raw_html) },
    };

    s.save(&name, content, force)?;

    if json {
        let out = serde_json::json!({
            "ok": true,
            "name": meta.name,
            "templatized": meta.templatized,
            "variable_count": meta.variables.len(),
            "tags": meta.tags,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!(
            "Captured template '{}' ({} variable{}).",
            name,
            variables.len(),
            if variables.len() == 1 { "" } else { "s" }
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_paste(
    name: Option<String>,
    data: Option<String>,
    data_file: Option<PathBuf>,
    stdin_flag: bool,
    dry_run: bool,
    plain_text: String,
    open: bool,
    from_table: bool,
    template_name: String,
    json: bool,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let templates_dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap().join(".config"))
        .join("clipli")
        .join("templates");

    let renderer = Renderer::new(&templates_dir)?;

    let rendered_html = if from_table {
        // Read TableInput JSON from stdin
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        let table: TableInput = serde_json::from_str(&buf)?;
        let table_value = serde_json::to_value(&table)?;
        let output = renderer.render(&template_name, &table_value)?;
        output.html
    } else {
        let tmpl_name = name.ok_or("template name is required unless --from-table is set")?;
        tracing::debug!(template = %tmpl_name, "paste: loading template");
        let s = Store::new()?;
        s.load(&tmpl_name)?; // validate template exists

        // Merge data
        let merged = merge_data(data, data_file, stdin_flag)?;

        let output = renderer.render(&tmpl_name, &merged)?;
        output.html
    };

    if dry_run {
        print!("{}", rendered_html);
        return Ok(());
    }

    // Determine plain text
    let plain = match plain_text.as_str() {
        "none" => None,
        "auto" => match config.defaults.plain_text_strategy.as_str() {
            "none" => None,
            _ => Some(render::html_to_plain_text(&rendered_html)),
        },
        _ => Some(render::html_to_plain_text(&rendered_html)),
    };

    if open {
        let preview_path = write_preview_file("paste", &rendered_html, None)?;
        open_in_browser(&preview_path)?;
    }

    pb::write_html(&rendered_html, plain.as_deref())?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "html_bytes": rendered_html.len(),
                "plain_bytes": plain.as_ref().map(|p| p.len()),
            })
        );
    }
    Ok(())
}

fn cmd_preview(
    name: Option<String>,
    input: Option<PathBuf>,
    data: Option<String>,
    data_file: Option<PathBuf>,
    open: bool,
    output: Option<PathBuf>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let html = if let Some(path) = input {
        std::fs::read_to_string(path)?
    } else if let Some(tmpl_name) = name {
        let renderer = Renderer::new(&templates_dir())?;
        let s = Store::new()?;
        s.load(&tmpl_name)?;
        let merged = merge_data(data, data_file, false)?;
        renderer.render(&tmpl_name, &merged)?.html
    } else {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        if buf.trim().is_empty() {
            return Err(
                "preview requires a template name, --input <file>, or HTML on stdin".into(),
            );
        }
        buf
    };

    let path = write_preview_file("preview", &html, output)?;
    if open {
        open_in_browser(&path)?;
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "path": path,
                "html_bytes": html.len(),
                "opened": open,
            }))?
        );
    } else {
        println!("{}", path.display());
    }
    Ok(())
}

fn cmd_list(
    tag: Option<String>,
    json: bool,
    detail: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let s = Store::new()?;
    let filter = if tag.is_some() {
        Some(ListFilter {
            tag,
            templatized_only: false,
        })
    } else {
        None
    };
    let metas = s.list(filter.as_ref())?;

    if json {
        println!("{}", serde_json::to_string_pretty(&metas)?);
    } else {
        println!("Templates ({}):", metas.len());
        for meta in &metas {
            let status = if meta.templatized {
                "templatized"
            } else {
                "raw"
            };
            let var_count = meta.variables.len();
            let tags_str = if meta.tags.is_empty() {
                String::new()
            } else {
                format!("[{}]", meta.tags.join(", "))
            };
            println!(
                "  {:<30}  {:<12}  {} var{}  {}",
                meta.name,
                status,
                var_count,
                if var_count == 1 { " " } else { "s" },
                tags_str
            );
            if detail && !meta.variables.is_empty() {
                for var in &meta.variables {
                    let desc = var
                        .description
                        .as_deref()
                        .map(|d| format!(" — {}", d))
                        .unwrap_or_default();
                    println!("      • {}{}", var.name, desc);
                }
            }
        }
    }
    Ok(())
}

fn cmd_show(
    name: String,
    html_flag: bool,
    schema_flag: bool,
    meta_flag: bool,
    open: bool,
    version: Option<String>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let s = Store::new()?;
    let loaded = if let Some(ref ver) = version {
        s.load_version(&name, ver)?
    } else {
        s.load(&name)?
    };

    if json {
        let out = serde_json::json!({
            "ok": true,
            "meta": loaded.meta,
            "schema": loaded.schema,
            "html_bytes": loaded.template_html.len(),
            "is_templatized": loaded.is_templatized,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if html_flag {
        print!("{}", loaded.template_html);
        return Ok(());
    }

    if schema_flag {
        println!("{}", serde_json::to_string_pretty(&loaded.schema)?);
        return Ok(());
    }

    if meta_flag {
        println!("{}", serde_json::to_string_pretty(&loaded.meta)?);
        return Ok(());
    }

    if open {
        let templates_dir = dirs::config_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap().join(".config"))
            .join("clipli")
            .join("templates");
        let renderer = Renderer::new(&templates_dir)?;
        // Build defaults data from schema
        let mut defaults = serde_json::Map::new();
        for var in &loaded.schema {
            if let Some(val) = &var.default_value {
                defaults.insert(var.name.clone(), val.clone());
            }
        }
        let data = serde_json::Value::Object(defaults);
        let output = renderer.render(&name, &data)?;
        let preview_path = write_preview_file("show", &output.html, None)?;
        open_in_browser(&preview_path)?;
        return Ok(());
    }

    // Default summary
    println!("Name:        {}", loaded.meta.name);
    if let Some(desc) = &loaded.meta.description {
        println!("Description: {}", desc);
    }
    println!(
        "Templatized: {}",
        if loaded.meta.templatized { "yes" } else { "no" }
    );
    println!("Variables:   {}", loaded.meta.variables.len());
    if !loaded.meta.tags.is_empty() {
        println!("Tags:        {}", loaded.meta.tags.join(", "));
    }
    if let Some(app) = &loaded.meta.source_app {
        println!("Source app:  {}", app);
    }
    println!("Created:     {}", loaded.meta.created_at);
    println!("Updated:     {}", loaded.meta.updated_at);
    Ok(())
}

fn cmd_edit(name: String, auto_schema: bool) -> Result<(), Box<dyn std::error::Error>> {
    let s = Store::new()?;

    let path = s
        .template_file_path(&name)
        .ok_or_else(|| format!("template '{}' not found", name))?;

    // Snapshot before editing
    let _ = s.snapshot(&name, "edit");

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    let status = std::process::Command::new(&editor).arg(&path).status()?;

    if !status.success() {
        return Err(format!("editor '{}' exited with non-zero status", editor).into());
    }

    // Read back the edited file
    let updated_html = std::fs::read_to_string(&path)?;

    // Detect new {{ variables }} via simple regex
    let var_re = regex::Regex::new(r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\}\}").unwrap();
    let found_vars: std::collections::HashSet<String> = var_re
        .captures_iter(&updated_html)
        .map(|c| c[1].to_string())
        .collect();

    // Load existing meta and schema
    let loaded = s.load(&name)?;
    let existing_var_names: std::collections::HashSet<String> =
        loaded.schema.iter().map(|v| v.name.clone()).collect();

    let new_vars: Vec<String> = found_vars
        .difference(&existing_var_names)
        .cloned()
        .collect();

    if !new_vars.is_empty() {
        if auto_schema {
            // Add new variables to schema
            let mut schema = loaded.schema.clone();
            for var_name in &new_vars {
                schema.push(model::TemplateVariable {
                    name: var_name.clone(),
                    var_type: model::VarType::String,
                    default_value: None,
                    description: None,
                });
            }
            let schema_path = s.template_dir(&name).join("schema.json");
            std::fs::write(&schema_path, serde_json::to_string_pretty(&schema)?)?;
            println!("Added {} new variable(s) to schema.", new_vars.len());
        } else {
            println!(
                "Detected {} new variable(s): {}. Use --auto-schema to add them.",
                new_vars.len(),
                new_vars.join(", ")
            );
        }
    }

    // Update updated_at in meta.json
    let mut meta = loaded.meta.clone();
    meta.updated_at = chrono::Utc::now();
    if auto_schema && !new_vars.is_empty() {
        // Reflect the discovered variables in meta too
        let existing_meta_names: std::collections::HashSet<String> =
            meta.variables.iter().map(|v| v.name.clone()).collect();
        for var_name in &new_vars {
            if !existing_meta_names.contains(var_name) {
                meta.variables.push(model::TemplateVariable {
                    name: var_name.clone(),
                    var_type: model::VarType::String,
                    default_value: None,
                    description: None,
                });
            }
        }
        meta.templatized = true;
    }
    let meta_path = s.template_dir(&name).join("meta.json");
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;

    Ok(())
}

fn cmd_delete(
    name: String,
    force: bool,
    keep_versions: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let s = Store::new()?;

    if json && !force {
        return Err("--json requires --force (cannot prompt interactively)".into());
    }

    if !force {
        use std::io::{BufRead, Write};
        print!("Delete template '{}'? [y/N] ", name);
        std::io::stdout().flush()?;
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        let answer = line.trim().to_lowercase();
        if answer != "y" && answer != "yes" {
            println!("Aborted.");
            return Ok(());
        }
    }

    if keep_versions {
        s.delete_preserving_versions(&name)?;
        if json {
            println!(
                "{}",
                serde_json::json!({"ok": true, "name": name, "deleted": true, "keep_versions": true})
            );
        } else {
            println!("Deleted template '{}' (version history preserved).", name);
        }
    } else {
        s.delete(&name)?;
        if json {
            println!(
                "{}",
                serde_json::json!({"ok": true, "name": name, "deleted": true})
            );
        } else {
            println!("Deleted template '{}'.", name);
        }
    }
    Ok(())
}

fn cmd_versions(name: String, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let s = Store::new()?;
    let versions = s.list_versions(&name)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&versions)?);
    } else if versions.is_empty() {
        println!("No versions found for '{}'.", name);
    } else {
        println!("Versions for '{}' ({}):", name, versions.len());
        for v in &versions {
            println!(
                "  {}  ({})  {}",
                v.id,
                v.change_type,
                v.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
            );
        }
    }
    Ok(())
}

fn cmd_restore(name: String, version: String) -> Result<(), Box<dyn std::error::Error>> {
    let s = Store::new()?;
    s.restore_version(&name, &version)?;
    println!("Restored '{}' from version {}.", name, version);
    Ok(())
}

fn cmd_lint(name: String, strict: bool, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let s = Store::new()?;
    let loaded = s.load(&name)?;
    let report = lint::lint(&loaded.template_html, &loaded.schema);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for d in &report.diagnostics {
            let prefix = match d.severity {
                lint::Severity::Error => "ERROR",
                lint::Severity::Warning => "WARN",
            };
            if let Some(line) = d.line {
                eprintln!("[{}] line {}: {} ({})", prefix, line, d.message, d.code);
            } else {
                eprintln!("[{}] {} ({})", prefix, d.message, d.code);
            }
            if let Some(ref ctx) = d.context {
                eprintln!("  | {}", ctx);
            }
        }
        println!(
            "{} error(s), {} warning(s)",
            report.error_count, report.warning_count
        );
    }

    if report.error_count > 0 || (strict && report.warning_count > 0) {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_search(
    query: String,
    tag: Option<String>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let s = Store::new()?;
    let results = s.search(&query, tag.as_deref())?;

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else if results.is_empty() {
        println!("No templates found matching '{}'.", query);
    } else {
        println!("Found {} result(s):", results.len());
        for r in &results {
            let desc = r.description.as_deref().unwrap_or("");
            println!("  {:<30}  [{}]  {}", r.name, r.match_field, desc);
            if !r.match_context.is_empty() {
                println!("    {}", r.match_context);
            }
        }
    }
    Ok(())
}

fn cmd_export(name: String, output: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let s = Store::new()?;
    let output_path = output.unwrap_or_else(|| PathBuf::from(format!("{}.clipli", name)));
    s.export(&name, &output_path)?;
    println!("Exported '{}' to {}", name, output_path.display());
    Ok(())
}

fn cmd_import(
    file: PathBuf,
    force: bool,
    name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let s = Store::new()?;
    let imported_name = s.import(&file, force, name.as_deref())?;
    println!("Imported template '{}'.", imported_name);
    Ok(())
}

fn cmd_render(
    name: String,
    data_file: PathBuf,
    output_dir: Option<PathBuf>,
    format: String,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Verify template exists
    let s = Store::new()?;
    s.load(&name)?;

    let templates_dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap().join(".config"))
        .join("clipli")
        .join("templates");
    let renderer = render::Renderer::new(&templates_dir)?;

    // Read data file
    let content = std::fs::read_to_string(&data_file)?;
    let rows: Vec<serde_json::Value> =
        if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
            arr
        } else {
            // Try newline-delimited JSON
            content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(serde_json::from_str)
                .collect::<Result<Vec<_>, _>>()?
        };

    if rows.is_empty() {
        return Err("data file contains no rows".into());
    }

    let results = renderer.render_batch(&name, &rows)?;

    if let Some(ref dir) = output_dir {
        std::fs::create_dir_all(dir)?;
        for (i, output) in results.iter().enumerate() {
            let ext = if format == "plain" { "txt" } else { "html" };
            let filename = format!("{:03}.{}", i + 1, ext);
            let content = if format == "plain" {
                &output.plain
            } else {
                &output.html
            };
            std::fs::write(dir.join(&filename), content)?;
        }
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "ok": true,
                    "rendered": results.len(),
                    "output_dir": dir.display().to_string(),
                })
            );
        } else {
            eprintln!("Rendered {} items to {}", results.len(), dir.display());
        }
    } else if json {
        let items: Vec<serde_json::Value> = results
            .iter()
            .enumerate()
            .map(|(i, o)| {
                serde_json::json!({
                    "index": i,
                    "html": o.html,
                    "plain": o.plain,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({"ok": true, "rendered": items.len(), "items": items})
        );
    } else {
        for (i, output) in results.iter().enumerate() {
            if i > 0 {
                println!("---");
            }
            let content = if format == "plain" {
                &output.plain
            } else {
                &output.html
            };
            print!("{}", content);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_excel_edit(
    set_values: Vec<String>,
    set_bgs: Vec<String>,
    set_fgs: Vec<String>,
    set_formats: Vec<String>,
    set_formulas: Vec<String>,
    set_aligns: Vec<String>,
    set_bolds: Vec<String>,
    set_italics: Vec<String>,
    set_wraps: Vec<String>,
    dry_run: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read HTML from clipboard
    let html_bytes = pb::read_type(PbType::Html)?;
    let html = String::from_utf8(html_bytes)?;

    // Parse all edit operations
    let mut edits: Vec<excel_edit::EditOp> = Vec::new();

    for spec in &set_values {
        edits.push(excel_edit::parse_set_value(spec)?);
    }
    for spec in &set_bgs {
        edits.push(excel_edit::parse_set_bg(spec)?);
    }
    for spec in &set_fgs {
        edits.push(excel_edit::parse_set_fg(spec)?);
    }
    for spec in &set_formats {
        edits.push(excel_edit::parse_set_format(spec)?);
    }
    for spec in &set_formulas {
        edits.push(excel_edit::parse_set_formula(spec)?);
    }
    for spec in &set_aligns {
        edits.push(excel_edit::parse_set_align(spec)?);
    }
    for spec in &set_bolds {
        edits.push(excel_edit::parse_set_bold(spec)?);
    }
    for spec in &set_italics {
        edits.push(excel_edit::parse_set_italic(spec)?);
    }
    for spec in &set_wraps {
        edits.push(excel_edit::parse_set_wrap(spec)?);
    }

    if edits.is_empty() {
        return Err("no edits specified".into());
    }

    // Apply edits
    let modified = excel_edit::apply_edits(&html, &edits);

    if dry_run {
        print!("{}", modified);
        return Ok(());
    }

    // Write back to clipboard
    let plain = render::html_to_plain_text(&modified);
    pb::write_html(&modified, Some(&plain))?;

    if json {
        println!("{}", serde_json::json!({"ok": true, "edits": edits.len()}));
    } else {
        eprintln!("Applied {} edit(s) to clipboard", edits.len());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_list_build(
    file: Option<PathBuf>,
    input_format: String,
    item_specs: Vec<String>,
    kind: Option<String>,
    ordered: bool,
    title: Option<String>,
    copy_as: String,
    out_file: Option<PathBuf>,
    font: Option<String>,
    font_size: Option<String>,
    class_name: Option<String>,
    tight: bool,
    sort: bool,
    dedupe: bool,
    dry_run: bool,
    json: bool,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let input_format = lists::InputFormat::parse(&input_format)?;
    let input = read_optional_text_input(file.as_ref())?;
    let fallback_kind = parse_list_kind(kind.as_deref(), ordered, lists::ListKind::Unordered)?;

    let mut doc = if let Some(input) = input {
        let mut parsed = lists::parse_document(&input, input_format)?;
        parsed.kind = parse_list_kind(kind.as_deref(), ordered, parsed.kind)?;
        parsed
    } else if !item_specs.is_empty() {
        lists::ListDocument::new(fallback_kind)
    } else {
        return Err(
            "list-build needs input: pipe Markdown/JSON/lines, pass a file, or use --item".into(),
        );
    };

    for spec in &item_specs {
        doc.add_path_item(spec)?;
    }
    if let Some(title) = title {
        doc.title = Some(title);
    }
    if sort {
        doc.sort_recursive();
    }
    if dedupe {
        doc.dedupe_recursive();
    }

    finish_list_output(
        &doc,
        ListOutputOptions {
            copy_as,
            out_file,
            font,
            font_size,
            class_name,
            tight,
            dry_run,
            json,
        },
        config,
    )
}

#[allow(clippy::too_many_arguments)]
fn cmd_list_edit(
    file: Option<PathBuf>,
    input_format: String,
    kind: Option<String>,
    ordered: bool,
    title: Option<String>,
    set_values: Vec<String>,
    append_items: Vec<String>,
    insert_before: Vec<String>,
    insert_after: Vec<String>,
    remove_paths: Vec<String>,
    check_paths: Vec<String>,
    uncheck_paths: Vec<String>,
    toggle_paths: Vec<String>,
    indent_paths: Vec<String>,
    outdent_paths: Vec<String>,
    sort_paths: Vec<String>,
    dedupe: bool,
    copy_as: String,
    out_file: Option<PathBuf>,
    font: Option<String>,
    font_size: Option<String>,
    class_name: Option<String>,
    tight: bool,
    dry_run: bool,
    json: bool,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let input_format = lists::InputFormat::parse(&input_format)?;
    let input = match read_optional_text_input(file.as_ref())? {
        Some(input) => input,
        None => read_list_from_clipboard(input_format)?,
    };
    let mut doc = lists::parse_document(&input, input_format)?;

    doc.kind = parse_list_kind(kind.as_deref(), ordered, doc.kind)?;
    if let Some(title) = title {
        doc.title = Some(title);
    }

    for spec in &set_values {
        lists::set_text(&mut doc, spec)?;
    }
    for spec in &append_items {
        lists::append_item(&mut doc, spec)?;
    }
    for spec in &insert_before {
        lists::insert_before(&mut doc, spec)?;
    }
    for spec in &insert_after {
        lists::insert_after(&mut doc, spec)?;
    }
    for path in &remove_paths {
        lists::remove_item(&mut doc, path)?;
    }
    for path in &check_paths {
        lists::set_checked(&mut doc, path, true)?;
    }
    for path in &uncheck_paths {
        lists::set_checked(&mut doc, path, false)?;
    }
    for path in &toggle_paths {
        lists::toggle_checked(&mut doc, path)?;
    }
    for path in &indent_paths {
        lists::indent_item(&mut doc, path)?;
    }
    for path in &outdent_paths {
        lists::outdent_item(&mut doc, path)?;
    }
    for path in &sort_paths {
        lists::sort_at(&mut doc, path)?;
    }
    if dedupe {
        doc.dedupe_recursive();
    }

    finish_list_output(
        &doc,
        ListOutputOptions {
            copy_as,
            out_file,
            font,
            font_size,
            class_name,
            tight,
            dry_run,
            json,
        },
        config,
    )
}

struct ListOutputOptions {
    copy_as: String,
    out_file: Option<PathBuf>,
    font: Option<String>,
    font_size: Option<String>,
    class_name: Option<String>,
    tight: bool,
    dry_run: bool,
    json: bool,
}

fn finish_list_output(
    doc: &lists::ListDocument,
    options: ListOutputOptions,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_format = lists::OutputFormat::parse(&options.copy_as)?;
    let render_options = lists::RenderOptions {
        font: options.font.unwrap_or_else(|| config.defaults.font.clone()),
        font_size: options
            .font_size
            .unwrap_or_else(|| format!("{}", config.defaults.font_size_pt)),
        class_name: options.class_name,
        tight: options.tight,
        include_metadata: true,
    };

    let markdown = lists::render_markdown(doc)?;
    let payload = match output_format {
        lists::OutputFormat::Html => lists::render_html(doc, &render_options)?,
        lists::OutputFormat::Markdown => markdown.clone(),
    };

    if options.dry_run {
        if let Some(path) = options.out_file {
            std::fs::write(&path, payload.as_bytes())?;
        } else {
            print!("{}", payload);
        }
        return Ok(());
    }

    match output_format {
        lists::OutputFormat::Html => pb::write_html(&payload, Some(&markdown))?,
        lists::OutputFormat::Markdown => pb::write(&[(PbType::PlainText, payload.as_bytes())])?,
    }

    if options.json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "items": doc.item_count(),
                "max_depth": doc.max_depth(),
                "format": options.copy_as.to_ascii_lowercase(),
                "bytes": payload.len(),
            })
        );
    } else {
        eprintln!(
            "Wrote {} list item{} as {} to clipboard",
            doc.item_count(),
            if doc.item_count() == 1 { "" } else { "s" },
            options.copy_as.to_ascii_lowercase()
        );
    }
    Ok(())
}

fn read_optional_text_input(
    file: Option<&PathBuf>,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match file {
        Some(path) if path.to_str() != Some("-") => Ok(Some(std::fs::read_to_string(path)?)),
        Some(_) => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            if buf.trim().is_empty() {
                Err("stdin input is empty".into())
            } else {
                Ok(Some(buf))
            }
        }
        None => {
            use std::io::{IsTerminal, Read};
            let mut stdin = std::io::stdin();
            if stdin.is_terminal() {
                return Ok(None);
            }
            let mut buf = String::new();
            stdin.read_to_string(&mut buf)?;
            if buf.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(buf))
            }
        }
    }
}

fn read_list_from_clipboard(
    input_format: lists::InputFormat,
) -> Result<String, Box<dyn std::error::Error>> {
    match input_format {
        lists::InputFormat::Html | lists::InputFormat::Auto => {
            if let Ok(data) = pb::read_type(PbType::Html) {
                return Ok(String::from_utf8(data)?);
            }
            Ok(String::from_utf8(pb::read_type(PbType::PlainText)?)?)
        }
        _ => Ok(String::from_utf8(pb::read_type(PbType::PlainText)?)?),
    }
}

fn parse_list_kind(
    value: Option<&str>,
    ordered: bool,
    fallback: lists::ListKind,
) -> Result<lists::ListKind, Box<dyn std::error::Error>> {
    if ordered {
        return Ok(lists::ListKind::Ordered);
    }
    match value {
        Some("ordered") => Ok(lists::ListKind::Ordered),
        Some("unordered") => Ok(lists::ListKind::Unordered),
        Some(other) => {
            Err(format!("unknown list kind '{other}': expected ordered or unordered").into())
        }
        None => Ok(fallback),
    }
}

#[derive(Debug, Default)]
struct ExcelPresetConfig {
    style: Option<String>,
    header_bg: Option<String>,
    header_fg: Option<String>,
    band_bg: Option<String>,
    font: Option<String>,
    col_specs: Vec<String>,
    align_specs: Vec<String>,
    bold_cols: Vec<String>,
    italic_cols: Vec<String>,
    wrap_cols: Vec<String>,
    fg_colors: Vec<String>,
    bg_colors: Vec<String>,
    color_rules: Vec<String>,
}

fn excel_preset_config(
    preset: Option<&str>,
) -> Result<ExcelPresetConfig, Box<dyn std::error::Error>> {
    let mut cfg = ExcelPresetConfig::default();
    match preset.unwrap_or("default") {
        "default" => {}
        "finance" => {
            cfg.header_bg = Some("#0F766E".to_string());
            cfg.header_fg = Some("#FFFFFF".to_string());
            cfg.band_bg = Some("#CCFBF1".to_string());
            cfg.font = Some("Aptos Display".to_string());
            cfg.col_specs = vec![
                "Revenue:currency:right".to_string(),
                "Cost:currency:right".to_string(),
                "Profit:currency:right".to_string(),
                "Margin:percent_1dp:right".to_string(),
            ];
            cfg.align_specs = vec!["Status:center".to_string()];
            cfg.bold_cols = vec!["Revenue".to_string(), "Profit".to_string()];
        }
        "executive" => {
            cfg.header_bg = Some("#172554".to_string());
            cfg.header_fg = Some("#F8FAFC".to_string());
            cfg.band_bg = Some("#E0E7FF".to_string());
            cfg.font = Some("Aptos Display".to_string());
            cfg.bold_cols = vec!["Status".to_string(), "Owner".to_string()];
            cfg.wrap_cols = vec!["Summary".to_string(), "Notes".to_string()];
        }
        "minimal" => {
            cfg.style = Some("plain".to_string());
            cfg.header_bg = Some("#F8FAFC".to_string());
            cfg.header_fg = Some("#111827".to_string());
            cfg.band_bg = Some("#FFFFFF".to_string());
            cfg.font = Some("Aptos".to_string());
        }
        "status" => {
            cfg.header_bg = Some("#1F2937".to_string());
            cfg.header_fg = Some("#FFFFFF".to_string());
            cfg.band_bg = Some("#F3F4F6".to_string());
            cfg.align_specs = vec!["Status:center".to_string()];
            cfg.bold_cols = vec!["Status".to_string()];
            cfg.color_rules = vec![
                "Status:contains:Done:#A0D771:#1B5E20".to_string(),
                "Status:contains:Blocked:#C92E25:#FFFFFF".to_string(),
                "Status:contains:Risk:#FCCF84:#6B4F00".to_string(),
            ];
        }
        other => return Err(format!("unknown Excel preset '{other}'").into()),
    }
    Ok(cfg)
}

fn merge_vecs(mut base: Vec<String>, mut extra: Vec<String>) -> Vec<String> {
    base.append(&mut extra);
    base
}

fn read_tabular_input(
    file: Option<&PathBuf>,
    input_format: &str,
) -> Result<excel::CsvData, Box<dyn std::error::Error>> {
    if let Some(path) = file {
        if input_format == "csv" && path.to_str() != Some("-") {
            return excel::read_csv(path);
        }
    }

    let raw = match file {
        Some(path) if path.to_str() != Some("-") => std::fs::read_to_string(path)?,
        _ => {
            use std::io::{IsTerminal, Read};

            let mut stdin = std::io::stdin();
            if stdin.is_terminal() {
                return Err(
                    "clipli excel needs input: pipe CSV/JSON, use a heredoc, pass a file path, or pass - for stdin"
                        .into(),
                );
            }

            let mut buf = String::new();
            stdin.read_to_string(&mut buf)?;
            if buf.trim().is_empty() {
                return Err(
                    "clipli excel received empty stdin; pipe CSV/JSON, use a heredoc, or pass a file path"
                        .into(),
                );
            }
            buf
        }
    };

    let format = match input_format {
        "auto" => {
            let trimmed = raw.trim_start();
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                "json"
            } else {
                "csv"
            }
        }
        other => other,
    };

    match format {
        "csv" => excel::read_csv_from_str(&raw),
        "json" => table_json_to_rows(&raw),
        other => Err(format!("unsupported --input-format '{other}'").into()),
    }
}

fn table_json_to_rows(data: &str) -> Result<excel::CsvData, Box<dyn std::error::Error>> {
    let value: serde_json::Value = serde_json::from_str(data)?;

    if value.get("rows").is_some() {
        if let Ok(table) = serde_json::from_value::<TableInput>(value.clone()) {
            let max_cols = table
                .headers
                .as_ref()
                .map(|headers| headers.len())
                .unwrap_or_else(|| table.rows.iter().map(|row| row.len()).max().unwrap_or(0));
            let headers = table
                .headers
                .unwrap_or_else(|| {
                    (1..=max_cols)
                        .map(|idx| format!("Column {idx}"))
                        .map(|value| model::Cell {
                            value,
                            style: model::CellStyle::default(),
                        })
                        .collect()
                })
                .into_iter()
                .map(|cell| cell.value)
                .collect();
            let rows = table
                .rows
                .into_iter()
                .map(|row| row.into_iter().map(|cell| cell.value).collect())
                .collect();
            return Ok((headers, rows));
        }

        let headers = value
            .get("headers")
            .and_then(|headers| headers.as_array())
            .ok_or("JSON table objects require a headers array when not using TableInput cells")?
            .iter()
            .map(json_cell_to_string)
            .collect::<Vec<_>>();
        let rows = value
            .get("rows")
            .and_then(|rows| rows.as_array())
            .ok_or("JSON table object requires a rows array")?
            .iter()
            .map(|row| {
                row.as_array()
                    .ok_or("JSON table rows must be arrays")
                    .map(|cells| cells.iter().map(json_cell_to_string).collect::<Vec<_>>())
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok((headers, rows));
    }

    let rows = value
        .as_array()
        .ok_or("JSON input must be a TableInput object, {headers, rows}, or an array of objects")?;
    let first = rows
        .first()
        .and_then(|row| row.as_object())
        .ok_or("JSON array input must contain objects")?;
    let headers = first.keys().cloned().collect::<Vec<_>>();
    let data_rows = rows
        .iter()
        .map(|row| {
            let obj = row
                .as_object()
                .ok_or("JSON array input must contain objects")?;
            Ok(headers
                .iter()
                .map(|header| obj.get(header).map(json_cell_to_string).unwrap_or_default())
                .collect::<Vec<_>>())
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    Ok((headers, data_rows))
}

fn json_cell_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(v) => v.to_string(),
        serde_json::Value::Number(v) => v.to_string(),
        other => other.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_excel(
    file: Option<PathBuf>,
    input_format: String,
    preset: Option<String>,
    style: Option<String>,
    header_bg: Option<String>,
    header_fg: Option<String>,
    band_bg: Option<String>,
    font: Option<String>,
    font_size: Option<String>,
    col_specs: Vec<String>,
    align_specs: Vec<String>,
    bold_cols: Vec<String>,
    italic_cols: Vec<String>,
    wrap_cols: Vec<String>,
    fg_colors: Vec<String>,
    bg_colors: Vec<String>,
    color_rules: Vec<String>,
    links: Vec<String>,
    copy_as: String,
    out_file: Option<PathBuf>,
    png_scale: f32,
    title: Option<String>,
    total_row: bool,
    total_formula: bool,
    formulas: Vec<String>,
    row_height: Option<u32>,
    header_height: Option<u32>,
    columns: Option<Vec<String>>,
    hidden_cols: Vec<String>,
    renames: Vec<String>,
    col_font_sizes: Vec<String>,
    dry_run: bool,
    json: bool,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let preset_config = excel_preset_config(preset.as_deref())?;
    let font = font
        .or_else(|| preset_config.font.clone())
        .unwrap_or_else(|| config.defaults.font.clone());
    let font_size = font_size.unwrap_or_else(|| format!("{}", config.defaults.font_size_pt));
    let style = style
        .or_else(|| preset_config.style.clone())
        .unwrap_or_else(|| "table".to_string());
    let header_bg = header_bg
        .or_else(|| preset_config.header_bg.clone())
        .unwrap_or_else(|| "#4472C4".to_string());
    let header_fg = header_fg
        .or_else(|| preset_config.header_fg.clone())
        .unwrap_or_else(|| "#FFFFFF".to_string());
    let band_bg = band_bg
        .or_else(|| preset_config.band_bg.clone())
        .unwrap_or_else(|| "#D9E1F2".to_string());
    let (headers, rows) = read_tabular_input(file.as_ref(), &input_format)?;

    // Build config
    let table_style = match style.as_str() {
        "plain" => excel::TableStyle::Plain,
        _ => excel::TableStyle::Table,
    };

    let mut col_formats = std::collections::HashMap::new();
    for spec in &preset_config.col_specs {
        let (name, fmt) = excel::parse_col_spec(spec);
        col_formats.insert(name, fmt);
    }
    for spec in &col_specs {
        let (name, fmt) = excel::parse_col_spec(spec);
        col_formats.insert(name, fmt);
    }

    let mut align_overrides = std::collections::HashMap::new();
    for spec in &preset_config.align_specs {
        let (name, align) = excel::parse_color_spec(spec);
        align_overrides.insert(name, align);
    }
    for spec in &align_specs {
        let (name, align) = excel::parse_color_spec(spec);
        align_overrides.insert(name, align);
    }

    let mut fg_map = std::collections::HashMap::new();
    for spec in &preset_config.fg_colors {
        let (name, color) = excel::parse_color_spec(spec);
        fg_map.insert(name, color);
    }
    for spec in &fg_colors {
        let (name, color) = excel::parse_color_spec(spec);
        fg_map.insert(name, color);
    }

    let mut bg_map = std::collections::HashMap::new();
    for spec in &preset_config.bg_colors {
        let (name, color) = excel::parse_color_spec(spec);
        bg_map.insert(name, color);
    }
    for spec in &bg_colors {
        let (name, color) = excel::parse_color_spec(spec);
        bg_map.insert(name, color);
    }

    let mut parsed_rules = Vec::new();
    for spec in &preset_config.color_rules {
        parsed_rules.push(excel::parse_color_rule(spec)?);
    }
    for spec in &color_rules {
        parsed_rules.push(excel::parse_color_rule(spec)?);
    }

    let mut link_map = std::collections::HashMap::new();
    for spec in &links {
        let (name, pattern) = excel::parse_color_spec(spec);
        link_map.insert(name, pattern);
    }

    let mut rename_map = std::collections::HashMap::new();
    for spec in &renames {
        let (old, new) = excel::parse_rename(spec);
        rename_map.insert(old, new);
    }

    let mut font_size_map = std::collections::HashMap::new();
    for spec in &col_font_sizes {
        let (name, size) = excel::parse_col_font_size(spec);
        font_size_map.insert(name, size);
    }

    let mut cell_formulas = std::collections::HashMap::new();
    for spec in &formulas {
        let (col, row, expr) = excel::parse_formula_spec(spec)?;
        cell_formulas.insert((col, row), expr);
    }

    let config = excel::ExcelConfig {
        style: table_style,
        header_bg,
        header_fg,
        band_bg,
        font,
        font_size,
        col_formats,
        bold_cols: merge_vecs(preset_config.bold_cols, bold_cols),
        italic_cols: merge_vecs(preset_config.italic_cols, italic_cols),
        wrap_cols: merge_vecs(preset_config.wrap_cols, wrap_cols),
        fg_colors: fg_map,
        bg_colors: bg_map,
        align_overrides,
        links: link_map,
        color_rules: parsed_rules,
        title,
        total_row,
        row_height,
        header_height,
        columns: columns.clone(),
        hidden_cols,
        renames: rename_map,
        col_font_sizes: font_size_map,
        total_formula,
        cell_formulas,
    };

    let copy_as = copy_as.to_ascii_lowercase();
    let visible_cols = config
        .columns
        .as_ref()
        .map(|c| c.len())
        .unwrap_or(headers.len());

    match copy_as.as_str() {
        "html" => {
            let html = excel::generate_html(&headers, &rows, &config);

            if dry_run {
                if let Some(path) = out_file {
                    std::fs::write(&path, html.as_bytes())?;
                } else {
                    print!("{}", html);
                }
                return Ok(());
            }

            let plain = render::html_to_plain_text(&html);
            pb::write_html(&html, Some(&plain))?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "rows": rows.len(),
                        "columns": visible_cols,
                        "format": "html",
                        "bytes": html.len(),
                    })
                );
            } else {
                eprintln!(
                    "Wrote {} rows × {} cols to clipboard ({})",
                    rows.len(),
                    visible_cols,
                    style
                );
            }
        }
        "svg" => {
            let svg = excel::generate_svg(&headers, &rows, &config);

            if dry_run {
                if let Some(path) = out_file {
                    std::fs::write(&path, svg.as_bytes())?;
                } else {
                    print!("{}", svg);
                }
                return Ok(());
            }

            pb::write(&[(PbType::Svg, svg.as_bytes())])?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "rows": rows.len(),
                        "columns": visible_cols,
                        "format": "svg",
                        "bytes": svg.len(),
                    })
                );
            } else {
                eprintln!(
                    "Wrote {} rows × {} cols as SVG to clipboard",
                    rows.len(),
                    visible_cols
                );
            }
        }
        "png" => {
            let svg = excel::generate_svg(&headers, &rows, &config);
            let png = excel::svg_to_png(&svg, png_scale)?;

            if dry_run {
                let path = out_file.ok_or("PNG dry-run requires --out-file <file>")?;
                std::fs::write(&path, &png)?;
                return Ok(());
            }

            pb::write(&[(PbType::Png, &png)])?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "rows": rows.len(),
                        "columns": visible_cols,
                        "format": "png",
                        "bytes": png.len(),
                    })
                );
            } else {
                eprintln!(
                    "Wrote {} rows × {} cols as PNG to clipboard",
                    rows.len(),
                    visible_cols
                );
            }
        }
        other => {
            return Err(format!(
                "unsupported --copy-as '{}': expected html, svg, or png",
                other
            )
            .into());
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_convert(
    from: String,
    to: String,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    data: Option<String>,
    strategy: String,
    json: bool,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read input
    let input_text = match input {
        Some(path) => std::fs::read_to_string(&path)?,
        None => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };

    let result: String = match (from.as_str(), to.as_str()) {
        ("html", "j2") => {
            let templatize_result = match strategy.as_str() {
                "agent" => {
                    let agent_cfg = templatize::AgentConfig {
                        command: config.agent.command.clone(),
                        args: config.agent.args.clone(),
                        timeout_secs: config.agent.timeout_secs,
                    };
                    templatize::agent(&input_text, None, &agent_cfg)?.template_html
                }
                _ => templatize::heuristic(&input_text).template_html,
            };
            templatize_result
        }
        ("j2", "html") => {
            // Render with provided data using an inline minijinja environment
            let render_data: serde_json::Value = if let Some(d) = data {
                serde_json::from_str(&d)?
            } else {
                serde_json::Value::Object(Default::default())
            };
            let mut env = minijinja::Environment::new();
            env.add_template_owned("_convert_inline", input_text)
                .map_err(|e| format!("template syntax error: {}", e))?;
            let tmpl = env
                .get_template("_convert_inline")
                .map_err(|e| format!("template error: {}", e))?;
            let ctx = minijinja::Value::from_serialize(&render_data);
            tmpl.render(ctx)
                .map_err(|e| format!("render error: {}", e))?
        }
        ("html", "plain") => render::html_to_plain_text(&input_text),
        ("rtf", "html") => rtf::rtf_to_html(input_text.as_bytes())?,
        _ => {
            return Err(format!("unsupported conversion: {} → {}", from, to).into());
        }
    };

    if json {
        if let Some(path) = output {
            std::fs::write(&path, result.as_bytes())?;
        }
        println!(
            "{}",
            serde_json::json!({"ok": true, "output_bytes": result.len()})
        );
    } else {
        match output {
            Some(path) => std::fs::write(&path, result.as_bytes())?,
            None => print!("{}", result),
        }
    }
    Ok(())
}

fn cmd_completions(shell: Shell) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "clipli", &mut std::io::stdout());
    Ok(())
}

fn cmd_history(command: HistoryCommand) -> Result<(), Box<dyn std::error::Error>> {
    let store = history::HistoryStore::new(history_dir());
    match command {
        HistoryCommand::Record {
            r#type,
            input,
            source_app,
            sensitive,
            json,
        } => {
            let pb_type = parse_pb_type(&r#type)?;
            let policy = parse_sensitive_policy(&sensitive)?;
            let (data, source_app) = match input {
                Some(path) => (std::fs::read(path)?, source_app),
                None => (pb::read_type(pb_type)?, source_app.or_else(pb::source_app)),
            };
            let entry = store.record(pb_type, &data, source_app, policy)?;
            print_history_entries(vec![entry], json)?;
        }
        HistoryCommand::List {
            limit,
            source_app,
            r#type,
            from,
            to,
            json,
        } => {
            let filter = build_history_filter(source_app, r#type, from, to)?;
            let entries = store
                .list_filtered(&filter)?
                .into_iter()
                .take(limit)
                .collect();
            print_history_entries(entries, json)?;
        }
        HistoryCommand::Search {
            query,
            limit,
            source_app,
            r#type,
            from,
            to,
            json,
        } => {
            let filter = build_history_filter(source_app, r#type, from, to)?;
            let entries = if filter.is_empty() {
                store.search(&query)?
            } else {
                store.search_filtered(&query, &filter)?
            }
            .into_iter()
            .take(limit)
            .collect();
            print_history_entries(entries, json)?;
        }
        HistoryCommand::Show { id, content, json } => {
            let entry = store.get(&id)?;
            if json {
                let content_value = if content && history::is_text_type(entry.pb_type) {
                    store
                        .payload(&entry)
                        .ok()
                        .map(|payload| String::from_utf8_lossy(&payload).to_string())
                } else {
                    None
                };
                let mut out = serde_json::json!({"ok": true, "entry": entry});
                if let Some(content) = content_value {
                    out["content"] = serde_json::Value::String(content);
                }
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                print_history_entry(&entry);
                if content {
                    let payload = store.payload(&entry)?;
                    if history::is_text_type(entry.pb_type) {
                        println!("\n{}", String::from_utf8_lossy(&payload));
                    } else {
                        println!("\n(binary payload: {} bytes)", payload.len());
                    }
                }
            }
        }
        HistoryCommand::Restore {
            id,
            dry_run,
            output,
            json,
        } => {
            let entry = store.get(&id)?;
            let payload = store.payload(&entry)?;
            if dry_run {
                if let Some(path) = output {
                    std::fs::write(path, &payload)?;
                } else if json {
                    // JSON mode must remain parseable; omit payload bytes unless --output is used.
                } else if history::is_text_type(entry.pb_type) {
                    print!("{}", String::from_utf8_lossy(&payload));
                } else {
                    return Err("binary history restore --dry-run requires --output <file>".into());
                }
            } else {
                pb::write(&[(entry.pb_type, &payload)])?;
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": true,
                        "id": entry.id,
                        "dry_run": dry_run,
                        "bytes": payload.len(),
                        "type": entry.uti,
                    }))?
                );
            }
        }
        HistoryCommand::Prune {
            keep_latest,
            before,
            source_app,
            r#type,
            from,
            to,
            dry_run,
            json,
        } => {
            let effective_to = before.or(to);
            let filter = build_history_filter(source_app, r#type, from, effective_to)?;
            let result = store.prune(&filter, keep_latest, dry_run)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": true,
                        "result": result,
                    }))?
                );
            } else {
                println!(
                    "{} history entr{} {} ({} kept)",
                    result.removed,
                    if result.removed == 1 { "y" } else { "ies" },
                    if dry_run {
                        "would be removed"
                    } else {
                        "removed"
                    },
                    result.kept
                );
            }
        }
    }
    Ok(())
}

fn cmd_watch(
    once: bool,
    max_items: Option<usize>,
    interval_ms: u64,
    max_history: Option<usize>,
    sensitive: String,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let policy = parse_sensitive_policy(&sensitive)?;
    let store = history::HistoryStore::new(history_dir());
    let mut recorded = Vec::new();
    let mut last_hash: Option<String> = None;

    loop {
        let snapshot = pb::read_all()?;
        let source_app = snapshot.source_app.clone();
        if let Some(entry) = snapshot
            .types
            .into_iter()
            .find(|entry| !matches!(entry.pb_type, PbType::Unknown))
        {
            let hash = history::sha256_hex(&entry.data);
            if last_hash.as_deref() != Some(hash.as_str()) {
                last_hash = Some(hash);
                let saved = store.record(entry.pb_type, &entry.data, source_app, policy)?;
                if !json {
                    eprintln!("recorded history entry {}", saved.id);
                }
                recorded.push(saved);
                if let Some(keep) = max_history {
                    let filter = history::HistoryFilter::default();
                    let _ = store.prune(&filter, Some(keep), false)?;
                }
            }
        }

        if once {
            break;
        }
        if max_items.map(|max| recorded.len() >= max).unwrap_or(false) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(interval_ms));
    }

    if json {
        print_history_entries(recorded, true)?;
    }
    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct DoctorCheck {
    name: &'static str,
    status: &'static str,
    message: String,
}

fn check_ok(name: &'static str, message: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        name,
        status: "ok",
        message: message.into(),
    }
}

fn check_warn(name: &'static str, message: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        name,
        status: "warn",
        message: message.into(),
    }
}

fn check_error(name: &'static str, message: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        name,
        status: "error",
        message: message.into(),
    }
}

fn check_skipped(name: &'static str, message: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        name,
        status: "skipped",
        message: message.into(),
    }
}

fn cmd_doctor(
    json: bool,
    skip_clipboard: bool,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut checks = Vec::new();

    checks.push(check_ok(
        "platform",
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    ));

    let config_path = config_file_path();
    if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(contents) => match toml::from_str::<Config>(&contents) {
                Ok(_) => checks.push(check_ok(
                    "config",
                    format!("loaded {}", config_path.display()),
                )),
                Err(e) => checks.push(check_error(
                    "config",
                    format!("could not parse {}: {}", config_path.display(), e),
                )),
            },
            Err(e) => checks.push(check_error(
                "config",
                format!("could not read {}: {}", config_path.display(), e),
            )),
        }
    } else {
        checks.push(check_warn(
            "config",
            format!(
                "no config file at {}; using defaults",
                config_path.display()
            ),
        ));
    }

    let templates_dir = templates_dir();
    match std::fs::create_dir_all(&templates_dir) {
        Ok(_) => {
            let probe = templates_dir.join(".clipli-doctor-write-test");
            match std::fs::write(&probe, b"ok").and_then(|_| std::fs::remove_file(&probe)) {
                Ok(_) => checks.push(check_ok(
                    "template_store",
                    format!("writable {}", templates_dir.display()),
                )),
                Err(e) => checks.push(check_error(
                    "template_store",
                    format!("not writable {}: {}", templates_dir.display(), e),
                )),
            }
        }
        Err(e) => checks.push(check_error(
            "template_store",
            format!("could not create {}: {}", templates_dir.display(), e),
        )),
    }

    match std::process::Command::new("textutil").arg("-help").output() {
        Ok(output)
            if output.status.success()
                || !output.stderr.is_empty()
                || !output.stdout.is_empty() =>
        {
            checks.push(check_ok("textutil", "available for RTF to HTML conversion"));
        }
        Ok(output) => checks.push(check_warn(
            "textutil",
            format!("found textutil but it exited with {}", output.status),
        )),
        Err(e) => checks.push(check_error("textutil", format!("not available: {}", e))),
    }

    if skip_clipboard {
        checks.push(check_skipped(
            "pasteboard",
            "clipboard check skipped by --skip-clipboard",
        ));
    } else {
        match pb::read_all() {
            Ok(snapshot) => checks.push(check_ok(
                "pasteboard",
                format!(
                    "read {} type(s) from the macOS pasteboard",
                    snapshot.types.len()
                ),
            )),
            Err(PbError::Empty) => checks.push(check_warn(
                "pasteboard",
                "pasteboard is reachable but currently empty",
            )),
            Err(e) => checks.push(check_error(
                "pasteboard",
                format!("could not read pasteboard: {}", e),
            )),
        }
    }

    match &config.agent.command {
        Some(cmd) => {
            let mut command = std::process::Command::new(cmd);
            command.args(["--help"]);
            match command.output() {
                Ok(_) => checks.push(check_ok(
                    "agent_command",
                    format!("configured command '{}' can be launched", cmd),
                )),
                Err(e) => checks.push(check_error(
                    "agent_command",
                    format!("configured command '{}' could not be launched: {}", cmd, e),
                )),
            }
        }
        None => checks.push(check_warn(
            "agent_command",
            "no external agent command configured; --strategy agent will use stdio protocol unless --agent-command is provided",
        )),
    }

    let has_errors = checks.iter().any(|check| check.status == "error");
    let has_warnings = checks.iter().any(|check| check.status == "warn");

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": !has_errors,
                "warnings": has_warnings,
                "checks": checks,
            }))?
        );
    } else {
        println!("clipli doctor");
        for check in &checks {
            println!(
                "  [{:<7}] {:<15} {}",
                check.status, check.name, check.message
            );
        }
        if has_errors {
            println!("Result: errors found");
        } else if has_warnings {
            println!("Result: usable with warnings");
        } else {
            println!("Result: ready");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap().join(".config"))
        .join("clipli")
}

fn history_dir() -> PathBuf {
    config_dir().join("history")
}

fn config_file_path() -> PathBuf {
    config_dir().join("config.toml")
}

fn templates_dir() -> PathBuf {
    config_dir().join("templates")
}

fn preview_dir() -> PathBuf {
    config_dir().join("previews")
}

fn write_preview_file(
    prefix: &str,
    html: &str,
    output: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = if let Some(path) = output {
        path
    } else {
        std::fs::create_dir_all(preview_dir())?;
        let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%3fZ");
        preview_dir().join(format!("{prefix}-{stamp}.html"))
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(&path, html)?;
    Ok(path)
}

/// Map a type string to PbType.
fn parse_pb_type(s: &str) -> Result<PbType, Box<dyn std::error::Error>> {
    match s.to_ascii_lowercase().as_str() {
        "html" => Ok(PbType::Html),
        "rtf" => Ok(PbType::Rtf),
        "plain" | "text" | "plaintext" => Ok(PbType::PlainText),
        "svg" => Ok(PbType::Svg),
        "png" => Ok(PbType::Png),
        "tiff" => Ok(PbType::Tiff),
        "pdf" => Ok(PbType::Pdf),
        other => Err(format!(
            "unknown pasteboard type '{}': use html, rtf, plain, svg, png, tiff, or pdf",
            other
        )
        .into()),
    }
}

fn parse_sensitive_policy(
    value: &str,
) -> Result<history::SensitivePolicy, Box<dyn std::error::Error>> {
    history::SensitivePolicy::parse(value).map_err(Into::into)
}

fn build_history_filter(
    source_app: Option<String>,
    pb_type: Option<String>,
    from: Option<String>,
    to: Option<String>,
) -> Result<history::HistoryFilter, Box<dyn std::error::Error>> {
    Ok(history::HistoryFilter {
        source_app,
        pb_type: pb_type.map(|value| parse_pb_type(&value)).transpose()?,
        from: from
            .as_deref()
            .map(|value| parse_history_datetime(value, false))
            .transpose()?,
        to: to
            .as_deref()
            .map(|value| parse_history_datetime(value, true))
            .transpose()?,
    })
}

fn parse_history_datetime(
    value: &str,
    end_of_day: bool,
) -> Result<chrono::DateTime<chrono::Utc>, Box<dyn std::error::Error>> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(value) {
        return Ok(dt.with_timezone(&chrono::Utc));
    }
    let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")?;
    let time = if end_of_day {
        chrono::NaiveTime::from_hms_milli_opt(23, 59, 59, 999).unwrap()
    } else {
        chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
    };
    Ok(chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
        date.and_time(time),
        chrono::Utc,
    ))
}

fn print_history_entries(
    entries: Vec<history::HistoryEntry>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "entries": entries,
            }))?
        );
    } else if entries.is_empty() {
        println!("No history entries");
    } else {
        for entry in &entries {
            print_history_entry(entry);
        }
    }
    Ok(())
}

fn print_history_entry(entry: &history::HistoryEntry) {
    let source = entry.source_app.as_deref().unwrap_or("-");
    let privacy = if entry.redacted { " redacted" } else { "" };
    println!(
        "{}  {}  {}  {} bytes  {}{}",
        entry.id, entry.captured_at, entry.uti, entry.size_bytes, source, privacy
    );
}

/// Map a target app string from config to TargetApp enum.
fn parse_target_app(s: &str) -> TargetApp {
    match s.to_ascii_lowercase().as_str() {
        "excel" => TargetApp::Excel,
        "powerpoint" | "ppt" => TargetApp::PowerPoint,
        "googlesheets" | "sheets" | "google_sheets" => TargetApp::GoogleSheets,
        _ => TargetApp::Generic,
    }
}

/// Simple HTML tag stripper for plain-text fallback.
#[allow(dead_code)]
fn strip_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
            }
            '>' => {
                in_tag = false;
            }
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Open a file in the default macOS application.
fn open_in_browser(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    std::process::Command::new("open").arg(path).status()?;
    Ok(())
}

/// Print a JSON error envelope to stdout.
fn print_json_error(message: &str, code: &str) {
    println!(
        "{}",
        serde_json::json!({"ok": false, "error": message, "code": code})
    );
}

/// Try to extract an error code from a boxed error by downcasting to known types.
fn try_error_code(e: &(dyn std::error::Error + 'static)) -> &'static str {
    if let Some(e) = e.downcast_ref::<pb::PbError>() {
        return e.code();
    }
    if let Some(e) = e.downcast_ref::<store::StoreError>() {
        return e.code();
    }
    if let Some(e) = e.downcast_ref::<render::RenderError>() {
        return e.code();
    }
    if let Some(e) = e.downcast_ref::<clean::CleanError>() {
        return e.code();
    }
    if let Some(e) = e.downcast_ref::<templatize::TemplatizeError>() {
        return e.code();
    }
    if let Some(e) = e.downcast_ref::<rtf::RtfError>() {
        return e.code();
    }
    "UNKNOWN"
}

/// Merge data from --data > --data_file > stdin.
fn merge_data(
    data_str: Option<String>,
    data_file: Option<PathBuf>,
    from_stdin: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if let Some(s) = data_str {
        return Ok(serde_json::from_str(&s)?);
    }
    if let Some(path) = data_file {
        let s = std::fs::read_to_string(&path)?;
        return Ok(serde_json::from_str(&s)?);
    }
    if from_stdin {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        return Ok(serde_json::from_str(&buf)?);
    }
    Ok(serde_json::Value::Object(Default::default()))
}

/// Format a u64 integer with comma grouping, e.g. 12847 → "12,847".
fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::new();
    let len = chars.len();
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*ch);
    }
    result
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.defaults.font, "Calibri");
        assert_eq!(config.defaults.font_size_pt, 11.0);
        assert_eq!(config.defaults.plain_text_strategy, "tab-delimited");
        assert!(!config.clean.keep_classes);
        assert_eq!(config.clean.target_app, "generic");
        assert_eq!(config.templatize.default_strategy, "heuristic");
    }

    #[test]
    fn config_override_defaults() {
        let config: Config = toml::from_str(
            r#"
[defaults]
font = "Arial"
font_size_pt = 14.0
plain_text_strategy = "none"

[clean]
keep_classes = true
target_app = "excel"

[templatize]
default_strategy = "agent"
"#,
        )
        .unwrap();
        assert_eq!(config.defaults.font, "Arial");
        assert_eq!(config.defaults.font_size_pt, 14.0);
        assert_eq!(config.defaults.plain_text_strategy, "none");
        assert!(config.clean.keep_classes);
        assert_eq!(config.clean.target_app, "excel");
        assert_eq!(config.templatize.default_strategy, "agent");
    }

    #[test]
    fn config_partial_sections() {
        let config: Config = toml::from_str(
            r#"
[defaults]
font = "Aptos Display"
"#,
        )
        .unwrap();
        assert_eq!(config.defaults.font, "Aptos Display");
        // Other fields should use defaults
        assert_eq!(config.defaults.font_size_pt, 11.0);
        assert_eq!(config.templatize.default_strategy, "heuristic");
    }

    #[test]
    fn config_empty_file() {
        let config: Config = toml::from_str("").unwrap();
        // All defaults should be populated
        assert_eq!(config.defaults.font, "Calibri");
        assert_eq!(config.clean.target_app, "generic");
    }
}
