mod clean;
mod excel;
mod excel_edit;
mod lint;
mod model;
mod pb;
mod render;
mod rtf;
mod store;
mod templatize;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

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
    about = "Clipboard intelligence CLI — template-driven pasteboard for agents and power users"
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
    /// Generate Excel-compatible HTML from CSV and write to clipboard
    Excel {
        /// CSV file path (or - for stdin)
        file: PathBuf,
        /// Table style: "table" (banded rows) or "plain" (thick borders)
        #[arg(long, default_value = "table")]
        style: String,
        /// Header background color
        #[arg(long, default_value = "#4472C4")]
        header_bg: String,
        /// Header text color
        #[arg(long, default_value = "#FFFFFF")]
        header_fg: String,
        /// Banded row background color (table style only)
        #[arg(long, default_value = "#D9E1F2")]
        band_bg: String,
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
        /// Print HTML to stdout instead of writing to clipboard
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
            | Commands::List { json: true, .. }
            | Commands::Paste { json: true, .. }
            | Commands::Show { json: true, .. }
            | Commands::Delete { json: true, .. }
            | Commands::Versions { json: true, .. }
            | Commands::Lint { json: true, .. }
            | Commands::Search { json: true, .. }
            | Commands::Excel { json: true, .. }
            | Commands::ExcelEdit { json: true, .. }
            | Commands::Render { json: true, .. }
            | Commands::Convert { json: true, .. }
            | Commands::Doctor { json: true, .. }
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
        let tmp_path = std::env::temp_dir().join("clipli_preview.html");
        std::fs::write(&tmp_path, &template_html)?;
        open_in_browser(&tmp_path)?;
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
        // Write to a temp file and open in browser
        let tmp_path = std::env::temp_dir().join("clipli_preview.html");
        std::fs::write(&tmp_path, &rendered_html)?;
        open_in_browser(&tmp_path)?;
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
        let tmp_path = std::env::temp_dir().join("clipli_preview.html");
        std::fs::write(&tmp_path, &output.html)?;
        open_in_browser(&tmp_path)?;
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
fn cmd_excel(
    file: PathBuf,
    style: String,
    header_bg: String,
    header_fg: String,
    band_bg: String,
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
    let font = font.unwrap_or_else(|| config.defaults.font.clone());
    let font_size = font_size.unwrap_or_else(|| format!("{}", config.defaults.font_size_pt));
    // Parse CSV
    let (headers, rows) = if file.to_str() == Some("-") {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        excel::read_csv_from_str(&buf)?
    } else {
        excel::read_csv(&file)?
    };

    // Build config
    let table_style = match style.as_str() {
        "plain" => excel::TableStyle::Plain,
        _ => excel::TableStyle::Table,
    };

    let mut col_formats = std::collections::HashMap::new();
    for spec in &col_specs {
        let (name, fmt) = excel::parse_col_spec(spec);
        col_formats.insert(name, fmt);
    }

    let mut align_overrides = std::collections::HashMap::new();
    for spec in &align_specs {
        let (name, align) = excel::parse_color_spec(spec);
        align_overrides.insert(name, align);
    }

    let mut fg_map = std::collections::HashMap::new();
    for spec in &fg_colors {
        let (name, color) = excel::parse_color_spec(spec);
        fg_map.insert(name, color);
    }

    let mut bg_map = std::collections::HashMap::new();
    for spec in &bg_colors {
        let (name, color) = excel::parse_color_spec(spec);
        bg_map.insert(name, color);
    }

    let mut parsed_rules = Vec::new();
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
        bold_cols,
        italic_cols,
        wrap_cols,
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

    let html = excel::generate_html(&headers, &rows, &config);

    if dry_run {
        print!("{}", html);
        return Ok(());
    }

    let plain = render::html_to_plain_text(&html);
    pb::write_html(&html, Some(&plain))?;

    let visible_cols = config
        .columns
        .as_ref()
        .map(|c| c.len())
        .unwrap_or(headers.len());
    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "rows": rows.len(),
                "columns": visible_cols,
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

fn config_file_path() -> PathBuf {
    config_dir().join("config.toml")
}

fn templates_dir() -> PathBuf {
    config_dir().join("templates")
}

/// Map a type string to PbType.
fn parse_pb_type(s: &str) -> Result<PbType, Box<dyn std::error::Error>> {
    match s.to_ascii_lowercase().as_str() {
        "html" => Ok(PbType::Html),
        "rtf" => Ok(PbType::Rtf),
        "plain" | "text" | "plaintext" => Ok(PbType::PlainText),
        "png" => Ok(PbType::Png),
        "tiff" => Ok(PbType::Tiff),
        "pdf" => Ok(PbType::Pdf),
        other => Err(format!(
            "unknown pasteboard type '{}': use html, rtf, plain, png, tiff, or pdf",
            other
        )
        .into()),
    }
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
