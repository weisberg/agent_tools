use std::fs;
use std::io::Read;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use docli_core::{DocliError, EnvelopeBuilder, Package};

use crate::envelope::emit;

#[derive(Subcommand)]
pub enum OoxmlCommand {
    /// Extract a DOCX to a directory with pretty-printed XML
    Unpack(UnpackArgs),
    /// Query document XML with an XPath-like expression
    Query(QueryArgs),
    /// Pack a directory back into a DOCX file
    Pack(PackArgs),
}

#[derive(Args)]
pub struct UnpackArgs {
    /// Path to the DOCX file
    file: PathBuf,
    /// Output directory
    #[arg(long)]
    dir: PathBuf,
}

#[derive(Args)]
pub struct QueryArgs {
    /// Path to the DOCX file
    file: PathBuf,
    /// XPath-like query (tag name to match)
    #[arg(long)]
    xpath: String,
}

#[derive(Args)]
pub struct PackArgs {
    /// Source directory containing unpacked DOCX parts
    dir: PathBuf,
    /// Output DOCX file path
    #[arg(long)]
    output: PathBuf,
}

#[derive(Serialize)]
struct UnpackData {
    file: String,
    output_dir: String,
    entries_written: usize,
}

#[derive(Serialize)]
struct QueryData {
    file: String,
    xpath: String,
    matches: Vec<MatchInfo>,
}

#[derive(Serialize)]
struct MatchInfo {
    tag: String,
    text: Option<String>,
    attributes: Vec<AttrInfo>,
}

#[derive(Serialize)]
struct AttrInfo {
    name: String,
    value: String,
}

#[derive(Serialize)]
struct PackData {
    source_dir: String,
    output: String,
    entries_written: usize,
}

pub fn run(command: OoxmlCommand, format: &str, pretty: bool) -> i32 {
    match command {
        OoxmlCommand::Unpack(args) => run_unpack(args, format, pretty),
        OoxmlCommand::Query(args) => run_query(args, format, pretty),
        OoxmlCommand::Pack(args) => run_pack(args, format, pretty),
    }
}

fn run_unpack(args: UnpackArgs, format: &str, pretty: bool) -> i32 {
    let mut builder = EnvelopeBuilder::new("ooxml.unpack");

    match execute_unpack(&args, &mut builder) {
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

fn execute_unpack(args: &UnpackArgs, _builder: &mut EnvelopeBuilder) -> Result<UnpackData, DocliError> {
    let package = Package::open(&args.file)?;

    fs::create_dir_all(&args.dir)?;

    let mut count = 0;

    // Write XML parts with pretty-printing
    for (name, bytes) in &package.xml_parts {
        let dest = args.dir.join(name);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let pretty_xml = pretty_print_xml(bytes);
        fs::write(&dest, pretty_xml)?;
        count += 1;
    }

    // Write binary parts from the original archive
    let mut archive = package.reopen_archive()?;
    for name in &package.binary_parts {
        let dest = args.dir.join(name);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut entry = archive
            .by_name(name)
            .map_err(|e| DocliError::CommitFailed {
                message: e.to_string(),
            })?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| DocliError::CommitFailed {
            message: e.to_string(),
        })?;
        fs::write(&dest, buf)?;
        count += 1;
    }

    Ok(UnpackData {
        file: args.file.display().to_string(),
        output_dir: args.dir.display().to_string(),
        entries_written: count,
    })
}

/// Best-effort XML pretty-printing: parse with roxmltree and indent.
/// Falls back to raw bytes if parsing fails.
fn pretty_print_xml(bytes: &[u8]) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    let Ok(doc) = roxmltree::Document::parse(text) else {
        return bytes.to_vec();
    };

    // Simple indentation approach: re-serialize by walking the tree
    let mut out = String::new();
    if let Some(pi) = text.find("?>") {
        // preserve XML declaration
        out.push_str(&text[..pi + 2]);
        out.push('\n');
    }
    walk_node(&doc.root(), &mut out, 0);
    out.into_bytes()
}

fn walk_node(node: &roxmltree::Node<'_, '_>, out: &mut String, depth: usize) {
    for child in node.children() {
        if child.is_element() {
            let indent = "  ".repeat(depth);
            out.push_str(&indent);
            out.push('<');
            if let Some(prefix) = child.tag_name().namespace().and_then(|ns| {
                child
                    .document()
                    .root_element()
                    .namespaces()
                    .find(|n| n.uri() == ns)
                    .and_then(|n| n.name())
            }) {
                out.push_str(prefix);
                out.push(':');
            }
            out.push_str(child.tag_name().name());
            for attr in child.attributes() {
                out.push(' ');
                out.push_str(attr.name());
                out.push_str("=\"");
                out.push_str(attr.value());
                out.push('"');
            }

            if !child.has_children() {
                out.push_str("/>\n");
            } else {
                out.push('>');
                // Check if this is a text-only element
                let children: Vec<_> = child.children().collect();
                if children.len() == 1 && children[0].is_text() {
                    if let Some(text) = children[0].text() {
                        out.push_str(text);
                    }
                    out.push_str("</");
                    if let Some(prefix) = child.tag_name().namespace().and_then(|ns| {
                        child
                            .document()
                            .root_element()
                            .namespaces()
                            .find(|n| n.uri() == ns)
                            .and_then(|n| n.name())
                    }) {
                        out.push_str(prefix);
                        out.push(':');
                    }
                    out.push_str(child.tag_name().name());
                    out.push_str(">\n");
                } else {
                    out.push('\n');
                    walk_node(&child, out, depth + 1);
                    out.push_str(&indent);
                    out.push_str("</");
                    if let Some(prefix) = child.tag_name().namespace().and_then(|ns| {
                        child
                            .document()
                            .root_element()
                            .namespaces()
                            .find(|n| n.uri() == ns)
                            .and_then(|n| n.name())
                    }) {
                        out.push_str(prefix);
                        out.push(':');
                    }
                    out.push_str(child.tag_name().name());
                    out.push_str(">\n");
                }
            }
        } else if child.is_text() {
            if let Some(text) = child.text() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push_str(&"  ".repeat(depth));
                    out.push_str(trimmed);
                    out.push('\n');
                }
            }
        }
    }
}

fn run_query(args: QueryArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("ooxml.query");

    match execute_query(&args) {
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

fn execute_query(args: &QueryArgs) -> Result<QueryData, DocliError> {
    let package = Package::open(&args.file)?;

    let doc_xml = package
        .xml_parts
        .get("word/document.xml")
        .ok_or_else(|| DocliError::InvalidDocx {
            message: "missing word/document.xml".to_string(),
        })?;

    let xml = std::str::from_utf8(doc_xml).map_err(|e| DocliError::InvalidDocx {
        message: e.to_string(),
    })?;
    let document = roxmltree::Document::parse(xml)?;

    // Simple tag-name matching (XPath-like: just match element names)
    let tag_name = args.xpath.trim_start_matches("//").trim_start_matches('/');

    let matches: Vec<MatchInfo> = document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == tag_name)
        .map(|node| {
            let text_content: String = node
                .descendants()
                .filter_map(|d| d.text())
                .collect();
            MatchInfo {
                tag: node.tag_name().name().to_string(),
                text: if text_content.is_empty() {
                    None
                } else {
                    Some(text_content)
                },
                attributes: node
                    .attributes()
                    .map(|a| AttrInfo {
                        name: a.name().to_string(),
                        value: a.value().to_string(),
                    })
                    .collect(),
            }
        })
        .collect();

    Ok(QueryData {
        file: args.file.display().to_string(),
        xpath: args.xpath.clone(),
        matches,
    })
}

fn run_pack(args: PackArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("ooxml.pack");

    match execute_pack(&args) {
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

fn execute_pack(args: &PackArgs) -> Result<PackData, DocliError> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    if !args.dir.exists() {
        return Err(DocliError::FileNotFound {
            path: args.dir.clone(),
        });
    }

    let file = fs::File::create(&args.output)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    let mut count = 0;

    // Walk the directory and add all files
    fn collect_files(
        dir: &PathBuf,
        base: &PathBuf,
        files: &mut Vec<(String, PathBuf)>,
    ) -> Result<(), DocliError> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path, base, files)?;
            } else {
                let relative = path
                    .strip_prefix(base)
                    .map_err(|e| DocliError::CommitFailed {
                        message: e.to_string(),
                    })?;
                files.push((relative.to_string_lossy().into_owned(), path.clone()));
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    collect_files(&args.dir, &args.dir, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, path) in &files {
        zip.start_file(name, options)
            .map_err(|e| DocliError::CommitFailed {
                message: e.to_string(),
            })?;
        let data = fs::read(path)?;
        zip.write_all(&data)?;
        count += 1;
    }

    zip.finish()
        .map_err(|e| DocliError::CommitFailed {
            message: e.to_string(),
        })?;

    Ok(PackData {
        source_dir: args.dir.display().to_string(),
        output: args.output.display().to_string(),
        entries_written: count,
    })
}
