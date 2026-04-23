use std::path::PathBuf;

use clap::Args;
use serde::Serialize;
use serde_json::{Map, Value};

use docli_core::{DocliError, EnvelopeBuilder};
use docli_create::{CreateSpec, DocxRsBackend};
use docli_kb::KbResolver;

use crate::envelope::emit;

#[derive(Args)]
pub struct CreateArgs {
    /// Path to the YAML spec file
    #[arg(long)]
    spec: PathBuf,
    /// Output DOCX file path
    #[arg(long)]
    out: PathBuf,
    /// Template variables as key=value pairs
    #[arg(long = "var", value_name = "KEY=VALUE")]
    vars: Vec<String>,
    /// KB root directory override
    #[arg(long)]
    kb_root: Option<PathBuf>,
}

#[derive(Serialize)]
struct CreateData {
    spec: String,
    out: String,
    vars_applied: usize,
    bytes_written: usize,
}

pub fn run(args: CreateArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("create");

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
            let envelope = builder.err::<serde_json::Value>(&e);
            let _ = emit(&envelope, format, pretty);
            1
        }
    }
}

fn parse_vars(raw: &[String]) -> Result<Map<String, Value>, DocliError> {
    let mut map = Map::new();
    for pair in raw {
        let (key, value) = pair.split_once('=').ok_or_else(|| DocliError::InvalidSpec {
            message: format!("invalid --var format (expected key=value): {pair}"),
        })?;
        map.insert(key.to_string(), Value::String(value.to_string()));
    }
    Ok(map)
}

fn make_resolver(kb_root: &Option<PathBuf>) -> Result<KbResolver, DocliError> {
    let root = match kb_root {
        Some(ref path) => path.clone(),
        None => {
            let env_root =
                std::env::var("DOCLI_KB_ROOT").map_err(|_| DocliError::DependencyMissing {
                    dependency: "DOCLI_KB_ROOT environment variable or --kb-root flag".to_string(),
                })?;
            PathBuf::from(env_root)
        }
    };
    Ok(KbResolver::new(root))
}

fn execute(args: &CreateArgs) -> Result<CreateData, DocliError> {
    // Read YAML spec file
    let yaml = std::fs::read_to_string(&args.spec).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            DocliError::FileNotFound {
                path: args.spec.clone(),
            }
        } else {
            DocliError::InvalidSpec {
                message: format!("failed to read spec file: {e}"),
            }
        }
    })?;

    // Parse --var overrides
    let vars = parse_vars(&args.vars)?;

    // Parse spec with variable substitution
    let mut spec = if vars.is_empty() {
        CreateSpec::from_yaml(&yaml)?
    } else {
        // Merge any vars already in the spec YAML with CLI overrides
        CreateSpec::from_yaml_with_vars(&yaml, &vars)?
    };

    // Resolve $refs via KbResolver (only if there are refs or kb_root is provided)
    let has_refs = spec.content.iter().any(|b| {
        matches!(b, docli_core::ContentBlock::Ref { .. })
    });
    if has_refs {
        let resolver = make_resolver(&args.kb_root)?;
        spec.resolve_refs(&resolver)?;
    }

    // Create DOCX via backend
    let backend = DocxRsBackend;
    let bytes = docli_create::CreateBackend::create(&backend, &spec)?;

    // Write output file
    std::fs::write(&args.out, &bytes).map_err(|e| DocliError::CommitFailed {
        message: format!("failed to write output file: {e}"),
    })?;

    Ok(CreateData {
        spec: args.spec.display().to_string(),
        out: args.out.display().to_string(),
        vars_applied: vars.len(),
        bytes_written: bytes.len(),
    })
}
