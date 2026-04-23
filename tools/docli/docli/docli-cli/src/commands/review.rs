use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use docli_core::{
    ContentBlock, Durability, EnvelopeBuilder, Job, Operation, ParagraphContent, PipelineHooks,
    PipelineRequest, Position, Target,
};

use crate::envelope::emit;

#[derive(Subcommand)]
pub enum ReviewCommand {
    /// Add a comment to targeted content
    Comment(CommentArgs),
    /// Tracked replacement of targeted content
    TrackReplace(TrackReplaceArgs),
    /// Tracked insertion before or after a target
    TrackInsert(TrackInsertArgs),
    /// Tracked deletion of targeted content
    TrackDelete(TrackDeleteArgs),
}

#[derive(Args)]
pub struct CommentArgs {
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
    /// Output DOCX file
    #[arg(long = "out")]
    output: PathBuf,
    /// Target selector (JSON)
    #[arg(long)]
    target: String,
    /// Comment text
    #[arg(long)]
    text: String,
}

#[derive(Args)]
pub struct TrackReplaceArgs {
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
    /// Output DOCX file
    #[arg(long = "out")]
    output: PathBuf,
    /// Target selector (JSON)
    #[arg(long)]
    target: String,
    /// Replacement text content
    #[arg(long)]
    content: String,
}

#[derive(Args)]
pub struct TrackInsertArgs {
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
    /// Output DOCX file
    #[arg(long = "out")]
    output: PathBuf,
    /// Target selector (JSON)
    #[arg(long)]
    target: String,
    /// Insert position relative to target
    #[arg(long)]
    position: PositionArg,
    /// Text content to insert
    #[arg(long)]
    content: String,
}

#[derive(Args)]
pub struct TrackDeleteArgs {
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
    /// Output DOCX file
    #[arg(long = "out")]
    output: PathBuf,
    /// Target selector (JSON)
    #[arg(long)]
    target: String,
}

#[derive(Clone, ValueEnum)]
pub enum PositionArg {
    Before,
    After,
}

impl From<PositionArg> for Position {
    fn from(p: PositionArg) -> Self {
        match p {
            PositionArg::Before => Position::Before,
            PositionArg::After => Position::After,
        }
    }
}

#[derive(Serialize)]
struct ReviewResult {
    source: String,
    output: String,
    operations: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

// ── Dispatch ────────────────────────────────────────────────────────

pub fn run(cmd: ReviewCommand, format: &str, pretty: bool) -> i32 {
    match cmd {
        ReviewCommand::Comment(args) => {
            let (input, output) = (args.input.clone(), args.output.clone());
            run_job(
                "review.comment",
                &input,
                &output,
                build_comment(args),
                format,
                pretty,
            )
        }
        ReviewCommand::TrackReplace(args) => {
            let (input, output) = (args.input.clone(), args.output.clone());
            run_job(
                "review.track-replace",
                &input,
                &output,
                build_track_replace(args),
                format,
                pretty,
            )
        }
        ReviewCommand::TrackInsert(args) => {
            let (input, output) = (args.input.clone(), args.output.clone());
            run_job(
                "review.track-insert",
                &input,
                &output,
                build_track_insert(args),
                format,
                pretty,
            )
        }
        ReviewCommand::TrackDelete(args) => {
            let (input, output) = (args.input.clone(), args.output.clone());
            run_job(
                "review.track-delete",
                &input,
                &output,
                build_track_delete(args),
                format,
                pretty,
            )
        }
    }
}

// ── Job builders ────────────────────────────────────────────────────

fn parse_target(raw: &str) -> Result<Target, String> {
    serde_json::from_str(raw).map_err(|e| format!("invalid target selector JSON: {e}"))
}

fn build_comment(args: CommentArgs) -> Result<Job, String> {
    let target = parse_target(&args.target)?;
    Ok(Job {
        operations: vec![Operation::ReviewComment {
            target,
            text: args.text,
            parent: None,
        }],
    })
}

fn build_track_replace(args: TrackReplaceArgs) -> Result<Job, String> {
    let target = parse_target(&args.target)?;
    Ok(Job {
        operations: vec![Operation::ReviewTrackReplace {
            target,
            content: args.content,
        }],
    })
}

fn build_track_insert(args: TrackInsertArgs) -> Result<Job, String> {
    let target = parse_target(&args.target)?;
    Ok(Job {
        operations: vec![Operation::ReviewTrackInsert {
            target,
            position: args.position.into(),
            content: vec![ContentBlock::Paragraph {
                paragraph: ParagraphContent::Text(args.content),
            }],
        }],
    })
}

fn build_track_delete(args: TrackDeleteArgs) -> Result<Job, String> {
    let target = parse_target(&args.target)?;
    Ok(Job {
        operations: vec![Operation::ReviewTrackDelete { target }],
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
            let envelope = builder.err::<ReviewResult>(&docli_core::DocliError::InvalidDocx {
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
            let data = ReviewResult {
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
            let envelope = builder.err::<ReviewResult>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}
