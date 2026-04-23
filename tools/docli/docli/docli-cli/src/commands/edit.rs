use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use docli_core::{
    ContentBlock, Durability, EnvelopeBuilder, Job, Operation, ParagraphContent, PipelineHooks,
    PipelineRequest, Position, Scope, Target,
};

use crate::envelope::emit;

#[derive(Subcommand)]
pub enum EditCommand {
    /// Replace targeted content with new text
    Replace(ReplaceArgs),
    /// Insert content before or after a target
    Insert(InsertArgs),
    /// Delete targeted content
    Delete(DeleteArgs),
    /// Find and replace text strings
    FindReplace(FindReplaceArgs),
}

// ── Subcommand arg structs ──────────────────────────────────────────

#[derive(Args)]
pub struct ReplaceArgs {
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
pub struct InsertArgs {
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
pub struct DeleteArgs {
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

#[derive(Args)]
pub struct FindReplaceArgs {
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
    /// Output DOCX file
    #[arg(long = "out")]
    output: PathBuf,
    /// Text to find
    #[arg(long)]
    find: String,
    /// Replacement text
    #[arg(long)]
    replace: String,
    /// Scope: all (default) or first
    #[arg(long, default_value = "all")]
    scope: ScopeArg,
}

// ── CLI-level enums that map to core types ──────────────────────────

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

#[derive(Clone, ValueEnum)]
pub enum ScopeArg {
    All,
    First,
}

impl From<ScopeArg> for Scope {
    fn from(s: ScopeArg) -> Self {
        match s {
            ScopeArg::All => Scope::All,
            ScopeArg::First => Scope::First,
        }
    }
}

// ── Result payload ──────────────────────────────────────────────────

#[derive(Serialize)]
struct EditResult {
    source: String,
    output: String,
    operations: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

// ── Dispatch ────────────────────────────────────────────────────────

pub fn run(cmd: EditCommand, format: &str, pretty: bool) -> i32 {
    match cmd {
        EditCommand::Replace(args) => {
            let (input, output) = (args.input.clone(), args.output.clone());
            run_job("edit.replace", &input, &output, build_replace(args), format, pretty)
        }
        EditCommand::Insert(args) => {
            let (input, output) = (args.input.clone(), args.output.clone());
            run_job("edit.insert", &input, &output, build_insert(args), format, pretty)
        }
        EditCommand::Delete(args) => {
            let (input, output) = (args.input.clone(), args.output.clone());
            run_job("edit.delete", &input, &output, build_delete(args), format, pretty)
        }
        EditCommand::FindReplace(args) => {
            let (input, output) = (args.input.clone(), args.output.clone());
            run_job("edit.find-replace", &input, &output, build_find_replace(args), format, pretty)
        }
    }
}

// ── Job builders ────────────────────────────────────────────────────

fn parse_target(raw: &str) -> Result<Target, String> {
    serde_json::from_str(raw).map_err(|e| format!("invalid target selector JSON: {e}"))
}

fn build_replace(args: ReplaceArgs) -> Result<Job, String> {
    let target = parse_target(&args.target)?;
    Ok(Job {
        operations: vec![Operation::EditReplace {
            target,
            content: args.content,
        }],
    })
}

fn build_insert(args: InsertArgs) -> Result<Job, String> {
    let target = parse_target(&args.target)?;
    Ok(Job {
        operations: vec![Operation::EditInsert {
            target,
            position: args.position.into(),
            content: vec![ContentBlock::Paragraph {
                paragraph: ParagraphContent::Text(args.content),
            }],
        }],
    })
}

fn build_delete(args: DeleteArgs) -> Result<Job, String> {
    let target = parse_target(&args.target)?;
    Ok(Job {
        operations: vec![Operation::EditDelete { target }],
    })
}

fn build_find_replace(args: FindReplaceArgs) -> Result<Job, String> {
    Ok(Job {
        operations: vec![Operation::EditFindReplace {
            find: args.find,
            replace: args.replace,
            scope: args.scope.into(),
        }],
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
            let envelope = builder.err::<EditResult>(&docli_core::DocliError::InvalidDocx {
                message: msg,
            });
            let _ = emit(&envelope, format, pretty);
            return 1;
        }
    };

    let op_count = job.operations.len();
    // Job is validated and counted; actual operation application will be wired
    // through PipelineHooks::apply_ops once the patch engine is integrated.
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
            let data = EditResult {
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
            let envelope = builder.err::<EditResult>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}
