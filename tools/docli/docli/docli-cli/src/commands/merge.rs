use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use docli_core::{Durability, EnvelopeBuilder, PipelineHooks, PipelineRequest};

use crate::envelope::emit;

#[derive(Args)]
pub struct MergeArgs {
    /// Base DOCX file
    #[arg(long)]
    base: PathBuf,
    /// Theirs DOCX file (incoming changes)
    #[arg(long)]
    theirs: PathBuf,
    /// Output DOCX file
    #[arg(long = "out")]
    output: PathBuf,
}

#[derive(Serialize)]
struct MergeResult {
    base: String,
    theirs: String,
    output: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

pub fn run(args: MergeArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("merge");

    // Stub: copy base to output (true merge is not yet implemented).
    let request = PipelineRequest {
        command: "merge".to_string(),
        source: args.base.clone(),
        output: args.output.clone(),
        durability: Durability::Durable,
        revalidate_after_write: false,
    };

    match docli_core::run_shadow_pipeline(&request, PipelineHooks::default()) {
        Ok(result) => {
            let mut warnings = result.warnings;
            warnings.push("merge is a stub: output is a copy of base".to_string());

            let data = MergeResult {
                base: args.base.display().to_string(),
                theirs: args.theirs.display().to_string(),
                output: result.output.display().to_string(),
                warnings,
            };
            let envelope = builder.ok(data);
            if let Err(e) = emit(&envelope, format, pretty) {
                eprintln!("output error: {e}");
                return 1;
            }
            0
        }
        Err(e) => {
            let envelope = builder.err::<MergeResult>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}
