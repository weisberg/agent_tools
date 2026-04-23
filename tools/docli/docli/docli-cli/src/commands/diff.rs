use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use docli_core::{DocliError, EnvelopeBuilder, Package};
use docli_query::DocumentIndex;
use docli_render::{semantic_diff, DiffSummary};

use crate::envelope::emit;

#[derive(Args)]
pub struct DiffArgs {
    /// First (old) DOCX file
    #[arg(long)]
    old: PathBuf,
    /// Second (new) DOCX file
    #[arg(long)]
    new: PathBuf,
}

#[derive(Serialize)]
struct DiffData {
    old_file: String,
    new_file: String,
    summary: DiffSummaryData,
    changes: Vec<DiffChangeData>,
}

#[derive(Serialize)]
struct DiffSummaryData {
    insertions: usize,
    deletions: usize,
    unchanged: usize,
}

#[derive(Serialize)]
struct DiffChangeData {
    tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    old_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_index: Option<usize>,
    value: String,
}

impl From<&DiffSummary> for DiffSummaryData {
    fn from(s: &DiffSummary) -> Self {
        DiffSummaryData {
            insertions: s.insertions,
            deletions: s.deletions,
            unchanged: s.unchanged,
        }
    }
}

pub fn run(args: DiffArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("diff");

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
            let envelope = builder.err::<DiffData>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}

fn execute(args: &DiffArgs) -> Result<DiffData, DocliError> {
    let old_pkg = Package::open(&args.old)?;
    let new_pkg = Package::open(&args.new)?;

    let old_doc_xml =
        old_pkg
            .xml_parts
            .get("word/document.xml")
            .ok_or_else(|| DocliError::InvalidDocx {
                message: "old file missing word/document.xml".to_string(),
            })?;

    let new_doc_xml =
        new_pkg
            .xml_parts
            .get("word/document.xml")
            .ok_or_else(|| DocliError::InvalidDocx {
                message: "new file missing word/document.xml".to_string(),
            })?;

    let old_index = DocumentIndex::build(old_doc_xml)?;
    let new_index = DocumentIndex::build(new_doc_xml)?;

    let result = semantic_diff(&old_index, &new_index);

    Ok(DiffData {
        old_file: args.old.display().to_string(),
        new_file: args.new.display().to_string(),
        summary: DiffSummaryData::from(&result.summary),
        changes: result
            .changes
            .iter()
            .map(|c| DiffChangeData {
                tag: c.tag.clone(),
                old_index: c.old_index,
                new_index: c.new_index,
                value: c.value.clone(),
            })
            .collect(),
    })
}
