use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use docli_core::{DocliError, EnvelopeBuilder, Package};
use docli_query::DocumentIndex;

use crate::envelope::emit;

#[derive(Args)]
pub struct ReadArgs {
    /// Path to the DOCX file
    file: PathBuf,
    /// Output rendering: text, json, markdown
    #[arg(long, default_value = "json")]
    render: String,
}

#[derive(Serialize)]
struct ReadData {
    file: String,
    paragraph_count: usize,
    content: serde_json::Value,
}

#[derive(Serialize)]
struct ParagraphJson {
    index: usize,
    style: Option<String>,
    text: String,
}

pub fn run(args: ReadArgs, format: &str, pretty: bool) -> i32 {
    let mut builder = EnvelopeBuilder::new("read");

    match execute(&args, &mut builder) {
        Ok(data) => {
            let envelope = builder.ok(data);
            if let Err(e) = emit(&envelope, format, pretty) {
                eprintln!("output error: {e}");
                return 1;
            }
            0
        }
        Err(e) => {
            let envelope = builder.err::<serde_json::Value>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}

fn execute(args: &ReadArgs, _builder: &mut EnvelopeBuilder) -> Result<ReadData, DocliError> {
    let package = Package::open(&args.file)?;

    let doc_xml = package
        .xml_parts
        .get("word/document.xml")
        .ok_or_else(|| DocliError::InvalidDocx {
            message: "missing word/document.xml".to_string(),
        })?;

    let rels_xml = package.xml_parts.get("word/_rels/document.xml.rels");
    let index = DocumentIndex::build_with_relationships(
        doc_xml,
        rels_xml.map(|v| v.as_slice()),
    )?;

    let content = match args.render.as_str() {
        "text" => render_text(&index),
        "markdown" => render_markdown(&index),
        _ => render_json(&index),
    };

    Ok(ReadData {
        file: args.file.display().to_string(),
        paragraph_count: index.paragraphs.len(),
        content,
    })
}

fn render_json(index: &DocumentIndex) -> serde_json::Value {
    let paragraphs: Vec<ParagraphJson> = index
        .paragraphs
        .iter()
        .map(|p| ParagraphJson {
            index: p.index,
            style: p.style.clone(),
            text: p.text.clone(),
        })
        .collect();
    serde_json::to_value(paragraphs).unwrap_or_default()
}

fn render_text(index: &DocumentIndex) -> serde_json::Value {
    let lines: Vec<String> = index
        .paragraphs
        .iter()
        .map(|p| format!("[{}] {}", p.index, p.text))
        .collect();
    serde_json::Value::String(lines.join("\n"))
}

fn render_markdown(index: &DocumentIndex) -> serde_json::Value {
    let mut out = String::new();
    for para in &index.paragraphs {
        // Check if this paragraph is a heading
        if let Some(heading) = index
            .headings
            .iter()
            .find(|h| h.paragraph_index == para.index)
        {
            let prefix = "#".repeat(heading.level as usize);
            out.push_str(&format!("{prefix} {}\n\n", heading.text));
        } else if !para.text.is_empty() {
            out.push_str(&para.text);
            out.push_str("\n\n");
        }
    }
    serde_json::Value::String(out)
}
