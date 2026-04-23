use std::io::Read as _;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use docli_core::{DocliError, EnvelopeBuilder, Package};
use docli_query::DocumentIndex;
use docli_render::index_to_text;

use crate::envelope::emit;

#[derive(Subcommand)]
pub enum ExtractCommand {
    /// Extract embedded images from a DOCX
    Images(ExtractImagesArgs),
    /// Extract plain text from a DOCX
    Text(ExtractTextArgs),
}

#[derive(Args)]
pub struct ExtractImagesArgs {
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
    /// Output directory for extracted images
    #[arg(long = "out-dir")]
    out_dir: PathBuf,
}

#[derive(Args)]
pub struct ExtractTextArgs {
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
}

#[derive(Serialize)]
struct ExtractImagesResult {
    source: String,
    out_dir: String,
    images_extracted: usize,
}

#[derive(Serialize)]
struct ExtractTextResult {
    source: String,
    text: String,
}

pub fn run(cmd: ExtractCommand, format: &str, pretty: bool) -> i32 {
    match cmd {
        ExtractCommand::Images(args) => run_images(args, format, pretty),
        ExtractCommand::Text(args) => run_text(args, format, pretty),
    }
}

fn run_images(args: ExtractImagesArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("extract.images");

    match extract_images(&args) {
        Ok(data) => {
            let envelope = builder.ok(data);
            if let Err(e) = emit(&envelope, format, pretty) {
                eprintln!("output error: {e}");
                return 1;
            }
            0
        }
        Err(e) => {
            let envelope = builder.err::<ExtractImagesResult>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}

fn run_text(args: ExtractTextArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("extract.text");

    match extract_text(&args) {
        Ok(data) => {
            let envelope = builder.ok(data);
            if let Err(e) = emit(&envelope, format, pretty) {
                eprintln!("output error: {e}");
                return 1;
            }
            0
        }
        Err(e) => {
            let envelope = builder.err::<ExtractTextResult>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}

fn extract_images(args: &ExtractImagesArgs) -> Result<ExtractImagesResult, DocliError> {
    let package = Package::open(&args.input)?;

    std::fs::create_dir_all(&args.out_dir).map_err(|e| DocliError::CommitFailed {
        message: format!("failed to create output directory: {e}"),
    })?;

    let mut count = 0;
    let mut archive = package.reopen_archive()?;

    let entry_names: Vec<String> = package.inventory.entries.keys().cloned().collect();
    for name in &entry_names {
        let is_image = name.starts_with("word/media/")
            && (name.ends_with(".png")
                || name.ends_with(".jpg")
                || name.ends_with(".jpeg")
                || name.ends_with(".gif")
                || name.ends_with(".bmp")
                || name.ends_with(".tiff")
                || name.ends_with(".emf")
                || name.ends_with(".wmf"));

        if is_image {
            let mut entry = archive.by_name(name)?;
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut bytes)
                .map_err(|e| DocliError::InvalidDocx {
                    message: e.to_string(),
                })?;
            drop(entry);

            let file_name = name.rsplit('/').next().unwrap_or(name);
            let dest = args.out_dir.join(file_name);
            std::fs::write(&dest, &bytes).map_err(|e| DocliError::CommitFailed {
                message: format!("failed to write image {}: {e}", dest.display()),
            })?;
            count += 1;
        }
    }

    Ok(ExtractImagesResult {
        source: args.input.display().to_string(),
        out_dir: args.out_dir.display().to_string(),
        images_extracted: count,
    })
}

fn extract_text(args: &ExtractTextArgs) -> Result<ExtractTextResult, DocliError> {
    let package = Package::open(&args.input)?;

    let doc_xml =
        package
            .xml_parts
            .get("word/document.xml")
            .ok_or_else(|| DocliError::InvalidDocx {
                message: "missing word/document.xml".to_string(),
            })?;

    let index = DocumentIndex::build(doc_xml)?;
    let text = index_to_text(&index);

    Ok(ExtractTextResult {
        source: args.input.display().to_string(),
        text,
    })
}
