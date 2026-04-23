use std::io::Read as _;
use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use docli_core::{
    Durability, EnvelopeBuilder, Job, PipelineHooks, PipelineRequest,
};

use crate::envelope::emit;

#[derive(Args)]
pub struct RunArgs {
    /// Path to a job file (YAML or JSON), or "-" to read from stdin
    job: String,
    /// Source DOCX file
    #[arg(long = "in")]
    input: PathBuf,
    /// Output DOCX file
    #[arg(long = "out")]
    output: PathBuf,
}

#[derive(Serialize)]
struct RunResult {
    source: String,
    output: String,
    operations: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

pub fn run(args: RunArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("run");

    let job = match load_job(&args.job) {
        Ok(j) => j,
        Err(msg) => {
            let envelope = builder.err::<RunResult>(&docli_core::DocliError::InvalidDocx {
                message: msg,
            });
            let _ = emit(&envelope, format, pretty);
            return 1;
        }
    };

    let op_count = job.operations.len();
    // Job is parsed and counted; actual operation application will be wired
    // through PipelineHooks::apply_ops once the patch engine is integrated.
    let _job = job;

    let request = PipelineRequest {
        command: "run".to_string(),
        source: args.input.clone(),
        output: args.output.clone(),
        durability: Durability::Durable,
        revalidate_after_write: false,
    };

    match docli_core::run_shadow_pipeline(&request, PipelineHooks::default()) {
        Ok(result) => {
            let data = RunResult {
                source: args.input.display().to_string(),
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
            let envelope = builder.err::<RunResult>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}

fn load_job(source: &str) -> Result<Job, String> {
    let raw = if source == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        buf
    } else {
        std::fs::read_to_string(source)
            .map_err(|e| format!("failed to read job file '{}': {e}", source))?
    };

    let trimmed = raw.trim_start();

    // Heuristic: if it starts with '{' or '[', try JSON first; otherwise YAML.
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        serde_json::from_str::<Job>(&raw)
            .map_err(|e| format!("invalid job JSON: {e}"))
    } else {
        serde_yaml::from_str::<Job>(&raw)
            .map_err(|e| format!("invalid job YAML: {e}"))
    }
}
