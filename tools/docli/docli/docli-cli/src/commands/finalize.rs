use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use docli_core::{
    Durability, EnvelopeBuilder, Job, Operation, PipelineHooks, PipelineRequest,
};

use crate::envelope::emit;

#[derive(Subcommand)]
pub enum FinalizeCommand {
    /// Accept tracked changes (all or by ID)
    Accept(AcceptArgs),
    /// Reject tracked changes (all or by ID)
    Reject(RejectArgs),
    /// Strip all tracked changes and comments
    Strip(StripArgs),
}

#[derive(Args)]
pub struct AcceptArgs {
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
    /// Output DOCX file
    #[arg(long = "out")]
    output: PathBuf,
    /// Comma-separated list of change IDs to accept (all if omitted)
    #[arg(long, value_delimiter = ',')]
    ids: Option<Vec<u64>>,
}

#[derive(Args)]
pub struct RejectArgs {
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
    /// Output DOCX file
    #[arg(long = "out")]
    output: PathBuf,
    /// Comma-separated list of change IDs to reject (all if omitted)
    #[arg(long, value_delimiter = ',')]
    ids: Option<Vec<u64>>,
}

#[derive(Args)]
pub struct StripArgs {
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
    /// Output DOCX file
    #[arg(long = "out")]
    output: PathBuf,
}

#[derive(Serialize)]
struct FinalizeResult {
    source: String,
    output: String,
    operations: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

// ── Dispatch ────────────────────────────────────────────────────────

pub fn run(cmd: FinalizeCommand, format: &str, pretty: bool) -> i32 {
    match cmd {
        FinalizeCommand::Accept(args) => {
            let (input, output) = (args.input.clone(), args.output.clone());
            run_job(
                "finalize.accept",
                &input,
                &output,
                build_accept(args),
                format,
                pretty,
            )
        }
        FinalizeCommand::Reject(args) => {
            let (input, output) = (args.input.clone(), args.output.clone());
            run_job(
                "finalize.reject",
                &input,
                &output,
                build_reject(args),
                format,
                pretty,
            )
        }
        FinalizeCommand::Strip(args) => {
            let (input, output) = (args.input.clone(), args.output.clone());
            run_job(
                "finalize.strip",
                &input,
                &output,
                Ok(Job {
                    operations: vec![Operation::FinalizeStrip {}],
                }),
                format,
                pretty,
            )
        }
    }
}

// ── Job builders ────────────────────────────────────────────────────

fn build_accept(args: AcceptArgs) -> Result<Job, String> {
    Ok(Job {
        operations: vec![Operation::FinalizeAccept { ids: args.ids }],
    })
}

fn build_reject(args: RejectArgs) -> Result<Job, String> {
    Ok(Job {
        operations: vec![Operation::FinalizeReject { ids: args.ids }],
    })
}

// ── Pipeline runner ─────────────────────────────────────────────────

fn run_job(
    label: &str,
    input: &PathBuf,
    output: &PathBuf,
    job_result: Result<Job, String>,
    format: &str,
    pretty: bool,
) -> i32 {
    let builder = EnvelopeBuilder::new(label);

    let job = match job_result {
        Ok(j) => j,
        Err(msg) => {
            let envelope =
                builder.err::<FinalizeResult>(&docli_core::DocliError::InvalidDocx {
                    message: msg,
                });
            let _ = emit(&envelope, format, pretty);
            return 1;
        }
    };

    let op_count = job.operations.len();
    let _job = job;

    let request = PipelineRequest {
        command: label.to_string(),
        source: input.clone(),
        output: output.clone(),
        durability: Durability::Durable,
        revalidate_after_write: false,
    };

    match docli_core::run_shadow_pipeline(&request, PipelineHooks::default()) {
        Ok(result) => {
            let data = FinalizeResult {
                source: input.display().to_string(),
                output: result.output.display().to_string(),
                operations: op_count,
                warnings: result.warnings,
            };
            let envelope = builder.ok(data);
            if let Err(e) = emit(&envelope, format, pretty) {
                eprintln!("output error: {e}");
                return 1;
            }
            0
        }
        Err(e) => {
            let envelope = builder.err::<FinalizeResult>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}
