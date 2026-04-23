use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::{Map, Value};

use docli_core::{DocliError, EnvelopeBuilder};
use docli_kb::KbResolver;

use crate::envelope::emit;

#[derive(Subcommand)]
pub enum TemplateCommand {
    /// List KB template entries in a category
    List(TemplateListArgs),
    /// Resolve and print a KB template by URI
    Get(TemplateGetArgs),
    /// Render a KB template with variable substitution
    Render(TemplateRenderArgs),
}

#[derive(Args)]
pub struct TemplateListArgs {
    /// Category name (e.g. styles, specs, sections)
    category: String,
    /// KB root directory override
    #[arg(long)]
    kb_root: Option<PathBuf>,
}

#[derive(Args)]
pub struct TemplateGetArgs {
    /// KB URI (e.g. kb://sections/cuped-methodology.yaml)
    uri: String,
    /// KB root directory override
    #[arg(long)]
    kb_root: Option<PathBuf>,
}

#[derive(Args)]
pub struct TemplateRenderArgs {
    /// KB URI (e.g. kb://specs/report.yaml)
    uri: String,
    /// Template variables as key=value pairs
    #[arg(long = "var", value_name = "KEY=VALUE")]
    vars: Vec<String>,
    /// KB root directory override
    #[arg(long)]
    kb_root: Option<PathBuf>,
}

#[derive(Serialize)]
struct TemplateListData {
    category: String,
    entries: Vec<String>,
}

#[derive(Serialize)]
struct TemplateGetData {
    uri: String,
    path: String,
    content: String,
}

#[derive(Serialize)]
struct TemplateRenderData {
    uri: String,
    vars_applied: usize,
    rendered: String,
}

pub fn run(command: TemplateCommand, format: &str, pretty: bool) -> i32 {
    match command {
        TemplateCommand::List(args) => run_list(args, format, pretty),
        TemplateCommand::Get(args) => run_get(args, format, pretty),
        TemplateCommand::Render(args) => run_render(args, format, pretty),
    }
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

fn run_list(args: TemplateListArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("template.list");

    match execute_list(&args) {
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

fn execute_list(args: &TemplateListArgs) -> Result<TemplateListData, DocliError> {
    let resolver = make_resolver(&args.kb_root)?;
    let entries = resolver.list_entries(&args.category)?;
    Ok(TemplateListData {
        category: args.category.clone(),
        entries,
    })
}

fn run_get(args: TemplateGetArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("template.get");

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

fn execute_get(args: &TemplateGetArgs) -> Result<TemplateGetData, DocliError> {
    let resolver = make_resolver(&args.kb_root)?;
    let path = resolver.resolve_uri(&args.uri)?;
    let content = std::fs::read_to_string(&path).map_err(|e| DocliError::TemplateNotFound {
        template: format!("{}: {}", args.uri, e),
    })?;
    Ok(TemplateGetData {
        uri: args.uri.clone(),
        path: path.display().to_string(),
        content,
    })
}

fn run_render(args: TemplateRenderArgs, format: &str, pretty: bool) -> i32 {
    let builder = EnvelopeBuilder::new("template.render");

    match execute_render(&args) {
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

fn execute_render(args: &TemplateRenderArgs) -> Result<TemplateRenderData, DocliError> {
    let resolver = make_resolver(&args.kb_root)?;
    let path = resolver.resolve_uri(&args.uri)?;
    let content = std::fs::read_to_string(&path).map_err(|e| DocliError::TemplateNotFound {
        template: format!("{}: {}", args.uri, e),
    })?;

    let vars = parse_vars(&args.vars)?;
    let rendered = docli_kb::render_template(&content, &vars)?;

    Ok(TemplateRenderData {
        uri: args.uri.clone(),
        vars_applied: vars.len(),
        rendered,
    })
}
