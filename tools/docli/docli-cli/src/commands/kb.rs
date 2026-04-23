use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use docli_core::{DocliError, EnvelopeBuilder};
use docli_kb::KbResolver;

use crate::envelope::emit;

#[derive(Subcommand)]
pub enum KbCommand {
    /// List entries in a KB category
    List(KbListArgs),
    /// Resolve a KB URI to a file path
    Get(KbGetArgs),
}

#[derive(Args)]
pub struct KbListArgs {
    /// Category name (e.g. styles, specs, sections)
    category: String,
    /// KB root directory override
    #[arg(long)]
    kb_root: Option<PathBuf>,
}

#[derive(Args)]
pub struct KbGetArgs {
    /// KB URI (e.g. kb://sections/cuped-methodology.yaml)
    uri: String,
    /// KB root directory override
    #[arg(long)]
    kb_root: Option<PathBuf>,
}

#[derive(Serialize)]
struct KbListData {
    category: String,
    entries: Vec<String>,
}

#[derive(Serialize)]
struct KbGetData {
    uri: String,
    path: String,
}

pub fn run(command: KbCommand, format: &str, pretty: bool) -> i32 {
    match command {
        KbCommand::List(args) => run_list(args, format, pretty),
        KbCommand::Get(args) => run_get(args, format, pretty),
    }
}

fn make_resolver(kb_root: &Option<PathBuf>) -> Result<KbResolver, DocliError> {
    let root = match kb_root {
        Some(ref path) => path.clone(),
        None => {
            let env_root = std::env::var("DOCLI_KB_ROOT").map_err(|_| {
                DocliError::DependencyMissing {
                    dependency: "DOCLI_KB_ROOT environment variable or --kb-root flag".to_string(),
                }
            })?;
            PathBuf::from(env_root)
        }
    };
    Ok(KbResolver::new(root))
}

fn run_list(args: KbListArgs, format: &str, pretty: bool) -> i32 {
    let mut builder = EnvelopeBuilder::new("kb.list");

    match execute_list(&args, &mut builder) {
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

fn execute_list(args: &KbListArgs, _builder: &mut EnvelopeBuilder) -> Result<KbListData, DocliError> {
    let resolver = make_resolver(&args.kb_root)?;
    let entries = resolver.list_entries(&args.category)?;
    Ok(KbListData {
        category: args.category.clone(),
        entries,
    })
}

fn run_get(args: KbGetArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("kb.get");

    match execute_get(&args) {
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

fn execute_get(args: &KbGetArgs) -> Result<KbGetData, DocliError> {
    let resolver = make_resolver(&args.kb_root)?;
    let path = resolver.resolve_uri(&args.uri)?;
    Ok(KbGetData {
        uri: args.uri.clone(),
        path: path.display().to_string(),
    })
}
