use std::collections::HashMap;
use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use docli_core::{DocliError, EnvelopeBuilder, Package};
use docli_query::DocumentIndex;

use crate::envelope::emit;

#[derive(Args)]
pub struct InspectArgs {
    /// Path to the DOCX file
    file: PathBuf,
    /// Comma-separated sections to include: paragraphs,headings,tables,images,bookmarks,comments,tracked_changes
    #[arg(long)]
    sections: Option<String>,
}

#[derive(Serialize)]
struct InspectData {
    file: String,
    source_hash: String,
    entry_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    paragraphs: Option<Vec<ParagraphInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    headings: Option<Vec<HeadingInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tables: Option<Vec<TableInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<ImageInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bookmarks: Option<HashMap<String, usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comments: Option<CommentInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tracked_changes: Option<TrackedChangeInfo>,
}

#[derive(Serialize)]
struct ParagraphInfo {
    index: usize,
    style: Option<String>,
    text: String,
}

#[derive(Serialize)]
struct HeadingInfo {
    paragraph_index: usize,
    level: u8,
    text: String,
}

#[derive(Serialize)]
struct TableInfo {
    index: usize,
    rows: usize,
    cols: usize,
}

#[derive(Serialize)]
struct ImageInfo {
    index: usize,
    paragraph_index: usize,
    relationship_id: String,
    target: Option<String>,
}

#[derive(Serialize)]
struct CommentInfo {
    count: usize,
}

#[derive(Serialize)]
struct TrackedChangeInfo {
    count: usize,
    insertions: usize,
    deletions: usize,
    authors: Vec<String>,
}

pub fn run(args: InspectArgs, format: &str, pretty: bool) -> i32 {
    let mut builder = EnvelopeBuilder::new("inspect");

    let result = execute(&args, &mut builder);
    match result {
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

fn execute(args: &InspectArgs, builder: &mut EnvelopeBuilder) -> Result<InspectData, DocliError> {
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

    let sections: Vec<String> = args
        .sections
        .as_deref()
        .map(|s| s.split(',').map(|s| s.trim().to_lowercase()).collect())
        .unwrap_or_else(|| {
            vec![
                "paragraphs".into(),
                "headings".into(),
                "tables".into(),
                "images".into(),
                "bookmarks".into(),
                "comments".into(),
                "tracked_changes".into(),
            ]
        });

    let include = |name: &str| sections.iter().any(|s| s == name);

    if index.paragraphs.is_empty() {
        builder.warn("document contains no paragraphs");
    }

    Ok(InspectData {
        file: args.file.display().to_string(),
        source_hash: package.source_hash.clone(),
        entry_count: package.entry_count(),
        paragraphs: if include("paragraphs") {
            Some(
                index
                    .paragraphs
                    .iter()
                    .map(|p| ParagraphInfo {
                        index: p.index,
                        style: p.style.clone(),
                        text: p.text.clone(),
                    })
                    .collect(),
            )
        } else {
            None
        },
        headings: if include("headings") {
            Some(
                index
                    .headings
                    .iter()
                    .map(|h| HeadingInfo {
                        paragraph_index: h.paragraph_index,
                        level: h.level,
                        text: h.text.clone(),
                    })
                    .collect(),
            )
        } else {
            None
        },
        tables: if include("tables") {
            Some(
                index
                    .tables
                    .iter()
                    .map(|t| TableInfo {
                        index: t.index,
                        rows: t.rows,
                        cols: t.cols,
                    })
                    .collect(),
            )
        } else {
            None
        },
        images: if include("images") {
            Some(
                index
                    .images
                    .iter()
                    .map(|i| ImageInfo {
                        index: i.index,
                        paragraph_index: i.paragraph_index,
                        relationship_id: i.relationship_id.clone(),
                        target: i.target.clone(),
                    })
                    .collect(),
            )
        } else {
            None
        },
        bookmarks: if include("bookmarks") {
            Some(index.bookmarks.clone())
        } else {
            None
        },
        comments: if include("comments") {
            Some(CommentInfo {
                count: index.comments.count,
            })
        } else {
            None
        },
        tracked_changes: if include("tracked_changes") {
            Some(TrackedChangeInfo {
                count: index.tracked_changes.count,
                insertions: index.tracked_changes.insertions,
                deletions: index.tracked_changes.deletions,
                authors: index.tracked_changes.authors.clone(),
            })
        } else {
            None
        },
    })
}
