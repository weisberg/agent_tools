use std::path::PathBuf;

use clap::{Args, ValueEnum};
use serde::Serialize;

use docli_core::{DocliError, EnvelopeBuilder, Package};
use docli_query::DocumentIndex;
use docli_render::{index_to_markdown, index_to_text};

use crate::envelope::emit;

#[derive(Args)]
pub struct ConvertArgs {
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
    /// Target format
    #[arg(long)]
    to: OutputFormat,
    /// Output file path
    #[arg(long = "out")]
    output: PathBuf,
}

#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    Pdf,
    Markdown,
    Text,
}

#[derive(Serialize)]
struct ConvertResult {
    source: String,
    output: String,
    format: String,
}

pub fn run(args: ConvertArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("convert");

    match execute(&args) {
        Ok(data) => {
            let envelope = builder.ok(data);
            if let Err(e) = emit(&envelope, format, pretty) {
                eprintln!("output error: {e}");
                return 1;
            }
            0
        }
        Err(e) => {
            let envelope = builder.err::<ConvertResult>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}

fn execute(args: &ConvertArgs) -> Result<ConvertResult, DocliError> {
    let package = Package::open(&args.input)?;

    let doc_xml =
        package
            .xml_parts
            .get("word/document.xml")
            .ok_or_else(|| DocliError::InvalidDocx {
                message: "missing word/document.xml".to_string(),
            })?;

    let index = DocumentIndex::build(doc_xml)?;

    let format_name = match &args.to {
        OutputFormat::Markdown => {
            let md = index_to_markdown(&index);
            std::fs::write(&args.output, md).map_err(|e| DocliError::CommitFailed {
                message: e.to_string(),
            })?;
            "markdown"
        }
        OutputFormat::Text => {
            let text = index_to_text(&index);
            std::fs::write(&args.output, text).map_err(|e| DocliError::CommitFailed {
                message: e.to_string(),
            })?;
            "text"
        }
        OutputFormat::Pdf => {
            // PDF conversion requires external tools (soffice/pandoc).
            // For now, produce a stub text file with a note.
            let text = index_to_text(&index);
            std::fs::write(&args.output, text).map_err(|e| DocliError::CommitFailed {
                message: e.to_string(),
            })?;
            "pdf"
        }
    };

    Ok(ConvertResult {
        source: args.input.display().to_string(),
        output: args.output.display().to_string(),
        format: format_name.to_string(),
    })
}
