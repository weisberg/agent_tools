mod client;
mod commands;
mod envelope;
mod protocol;
mod sidecar;
mod units;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "deckli", version, about = "Live PowerPoint control from the terminal")]
struct Cli {
    /// WebSocket bridge address
    #[arg(long, default_value = "ws://127.0.0.1:9716", global = true)]
    bridge: String,

    /// Output raw JSON (default for non-TTY)
    #[arg(long, global = true)]
    json: bool,

    /// Pretty-print output for humans
    #[arg(long, global = true)]
    pretty: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check bridge + add-in connection status
    Status,

    /// Start sidecar bridge and verify add-in connection
    Connect,

    /// Inspect presentation structure
    Inspect {
        /// Show master slides and layouts
        #[arg(long)]
        masters: bool,

        /// Show theme colors and fonts
        #[arg(long)]
        theme: bool,

        /// Inspect a specific slide's shapes
        #[arg(long)]
        slide: Option<u32>,
    },

    /// Read presentation content by path
    Get {
        /// Resource path (e.g. /slides/3, /slides/3/shapes/2, /selection)
        path: String,
    },

    /// Modify shape or slide properties
    Set {
        /// Resource path (e.g. /slides/3/shapes/title/text)
        path: String,

        /// Value to set (text content, color hex, etc.)
        value: Option<String>,

        // Font options
        /// Font size in points
        #[arg(long)]
        size: Option<f64>,

        /// Bold
        #[arg(long)]
        bold: Option<bool>,

        /// Italic
        #[arg(long)]
        italic: Option<bool>,

        // Geometry options
        /// Left position (e.g. 1in, 72pt, 2.54cm)
        #[arg(long)]
        left: Option<String>,

        /// Top position
        #[arg(long)]
        top: Option<String>,

        /// Width
        #[arg(long)]
        width: Option<String>,

        /// Height
        #[arg(long)]
        height: Option<String>,
    },

    /// Add slides, shapes, images, or tables
    Add {
        #[command(subcommand)]
        target: AddTarget,
    },

    /// Remove slides or shapes
    Rm {
        /// Resource path (e.g. /slides/5, /slides/3/shapes/2)
        path: String,
    },

    /// Reorder slides
    Move {
        /// Resource path (e.g. /slides/5)
        path: String,

        /// Destination index (1-based)
        #[arg(long)]
        to: u32,
    },

    /// Render slide(s) to PNG
    Render {
        /// Slide index (1-based)
        #[arg(long)]
        slide: Option<u32>,

        /// Render all slides
        #[arg(long)]
        all: bool,

        /// Output file or directory
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Execute a batch of commands atomically
    Batch {
        /// JSON file with command array
        #[arg(long)]
        file: Option<PathBuf>,

        /// Read command array from stdin
        #[arg(long)]
        stdin: bool,
    },

    /// Run as MCP server (stdio JSON-RPC)
    McpServe,
}

#[derive(Subcommand)]
enum AddTarget {
    /// Add a new slide from a layout
    Slide {
        /// Layout name (e.g. "Title Slide", "Two Content")
        #[arg(long)]
        layout: String,

        /// Insert position (1-based, default: end)
        #[arg(long)]
        at: Option<u32>,
    },

    /// Add a geometric shape
    Shape {
        /// Target slide index (1-based)
        #[arg(long)]
        slide: u32,

        /// Shape type (rectangle, ellipse, textbox, etc.)
        #[arg(long, name = "type")]
        shape_type: String,

        /// Left position (e.g. 1in, 72pt)
        #[arg(long)]
        left: String,

        /// Top position
        #[arg(long)]
        top: String,

        /// Width
        #[arg(long)]
        width: String,

        /// Height
        #[arg(long)]
        height: String,

        /// Fill color (hex, e.g. #2E75B6)
        #[arg(long)]
        fill: Option<String>,

        /// Text content
        #[arg(long)]
        text: Option<String>,
    },

    /// Add an image from a file
    Image {
        /// Target slide index (1-based)
        #[arg(long)]
        slide: u32,

        /// Source image file path
        #[arg(long)]
        src: PathBuf,

        /// Left position
        #[arg(long)]
        left: String,

        /// Top position
        #[arg(long)]
        top: String,

        /// Width
        #[arg(long)]
        width: String,

        /// Height
        #[arg(long)]
        height: String,
    },

    /// Add a table
    Table {
        /// Target slide index (1-based)
        #[arg(long)]
        slide: u32,

        /// Table data as JSON 2D array
        #[arg(long)]
        data: String,

        /// Left position
        #[arg(long)]
        left: String,

        /// Top position
        #[arg(long)]
        top: String,

        /// Width
        #[arg(long)]
        width: String,

        /// Height
        #[arg(long)]
        height: String,
    },
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("deckli=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Command::Status => commands::status::run(&cli.bridge).await,
        Command::Connect => commands::connect::run(&cli.bridge).await,
        Command::Inspect { masters, theme, slide } => {
            commands::inspect::run(&cli.bridge, masters, theme, slide).await
        }
        Command::Get { path } => commands::get::run(&cli.bridge, &path).await,
        Command::Set { path, value, size, bold, italic, left, top, width, height } => {
            commands::set::run(
                &cli.bridge, &path, value.as_deref(),
                size, bold, italic,
                left.as_deref(), top.as_deref(),
                width.as_deref(), height.as_deref(),
            ).await
        }
        Command::Add { target } => commands::add::run(&cli.bridge, target).await,
        Command::Rm { path } => commands::rm::run(&cli.bridge, &path).await,
        Command::Move { path, to } => commands::mv::run(&cli.bridge, &path, to).await,
        Command::Render { slide, all, out } => {
            commands::render::run(&cli.bridge, slide, all, out.as_deref()).await
        }
        Command::Batch { file, stdin } => {
            commands::batch::run(&cli.bridge, file.as_deref(), stdin).await
        }
        Command::McpServe => commands::mcp::run().await,
    };

    let output_json = cli.json || !atty::is(atty::Stream::Stdout);
    match result {
        Ok(value) => {
            if cli.pretty {
                println!("{}", serde_json::to_string_pretty(&value).unwrap());
            } else if output_json {
                println!("{}", serde_json::to_string(&value).unwrap());
            } else {
                // Human-friendly default
                println!("{}", serde_json::to_string_pretty(&value).unwrap());
            }
        }
        Err(e) => {
            let err = envelope::error_envelope("", &e);
            if cli.pretty || !output_json {
                eprintln!("{}", serde_json::to_string_pretty(&err).unwrap());
            } else {
                eprintln!("{}", serde_json::to_string(&err).unwrap());
            }
            std::process::exit(1);
        }
    }
}
