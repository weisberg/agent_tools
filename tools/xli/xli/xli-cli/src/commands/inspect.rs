use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use xli_core::{CommitMode, CommitStats, ResponseEnvelope, Status};
use xli_read::WorkbookInfo;

use crate::output;

#[derive(Debug, Args)]
pub struct InspectArgs {
    pub file: PathBuf,
}

pub fn run(args: InspectArgs, human: bool) -> Result<bool> {
    let envelope = match xli_read::inspect(&args.file) {
        Ok(info) => ok_envelope(args.file, info),
        Err(error) => error_envelope(args.file, error),
    };

    output::emit(&envelope, human)
}

fn ok_envelope(file: PathBuf, info: WorkbookInfo) -> ResponseEnvelope<WorkbookInfo> {
    ResponseEnvelope {
        status: Status::Ok,
        command: "inspect".to_string(),
        input: Some(serde_json::json!({ "file": file })),
        output: Some(info),
        commit_mode: CommitMode::None,
        fingerprint_before: None,
        fingerprint_after: None,
        needs_recalc: false,
        stats: CommitStats::default(),
        warnings: Vec::new(),
        errors: Vec::new(),
        suggested_repairs: Vec::new(),
    }
}

fn error_envelope(file: PathBuf, error: xli_core::XliError) -> ResponseEnvelope<WorkbookInfo> {
    ResponseEnvelope {
        status: Status::Error,
        command: "inspect".to_string(),
        input: Some(serde_json::json!({ "file": file })),
        output: None,
        commit_mode: CommitMode::None,
        fingerprint_before: None,
        fingerprint_after: None,
        needs_recalc: false,
        stats: CommitStats::default(),
        warnings: Vec::new(),
        errors: vec![error],
        suggested_repairs: Vec::new(),
    }
}
