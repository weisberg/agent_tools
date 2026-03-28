use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde_json::{json, Map, Value};
use vaultli::error::VaultliError;
use vaultli::vault::{
    add_file, build_index, find_root, infer_frontmatter, init_vault, load_index_records, make_id,
    scaffold_file, search_records, show_record, validate_vault,
};

#[derive(Parser)]
#[command(
    name = "vaultli",
    version,
    about = "Rust preview of the vaultli knowledge-vault CLI"
)]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Root {
        path: Option<PathBuf>,
    },
    Init {
        path: Option<PathBuf>,
    },
    MakeId {
        file: PathBuf,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Infer {
        file: PathBuf,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Index {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        full: bool,
    },
    Search {
        query: Option<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        jq: Option<String>,
    },
    Show {
        id: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Scaffold {
        file: PathBuf,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Add {
        file: PathBuf,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Validate {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    DumpIndex {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    let exit_code = match run(cli) {
        Ok(code) => code,
        Err((error, as_json)) => {
            emit_error(&error, as_json);
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(cli: Cli) -> Result<i32, (VaultliError, bool)> {
    let as_json = cli.json;
    match cli.command {
        Commands::Root { path } => {
            let root = find_root(path.as_deref()).map_err(|error| (error, as_json))?;
            emit_result(json!({ "root": root.display().to_string() }), as_json);
        }
        Commands::Init { path } => {
            let path = path.unwrap_or_else(|| PathBuf::from("."));
            let result = init_vault(&path).map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::MakeId { file, root } => {
            let result = make_id(&file, &root).map_err(|error| (error, as_json))?;
            emit_result(json!({ "id": result }), as_json);
        }
        Commands::Infer { file, root } => {
            let result = infer_frontmatter(&file, &root).map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Index { root, full } => {
            let result = build_index(&root, full).map_err(|error| (error, as_json))?;
            emit_result(serde_json::to_value(result).unwrap(), as_json);
        }
        Commands::Search { query, root, jq } => {
            let result = search_records(&root, query.as_deref(), jq.as_deref())
                .map_err(|error| (error, as_json))?;
            emit_result(json!({ "results": result, "total": result.len() }), as_json);
        }
        Commands::Show { id, root } => {
            let result = show_record(&root, &id).map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Scaffold { file, root } => {
            let result = scaffold_file(&root, &file).map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Add { file, root } => {
            let result = add_file(&root, &file).map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Validate { root } => {
            let result = validate_vault(&root).map_err(|error| (error, as_json))?;
            let exit_code = if result.valid { 0 } else { 1 };
            emit_result(serde_json::to_value(result).unwrap(), as_json);
            return Ok(exit_code);
        }
        Commands::DumpIndex { root } => {
            let records = load_index_records(&root).map_err(|error| (error, as_json))?;
            emit_result(json!({ "records": records }), as_json);
        }
    }
    Ok(0)
}

fn emit_result(value: Value, as_json: bool) {
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "ok": true, "result": value })).unwrap()
        );
        return;
    }

    match value {
        Value::Object(map) => print_map(&map),
        other => println!("{other}"),
    }
}

fn emit_error(error: &VaultliError, as_json: bool) {
    if as_json {
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": false,
                "error": {
                    "code": error.code(),
                    "message": error.to_string(),
                }
            }))
            .unwrap()
        );
        return;
    }
    eprintln!("error [{}]: {}", error.code(), error);
}

fn print_map(map: &Map<String, Value>) {
    for (key, value) in map {
        match value {
            Value::Array(items) => {
                let rendered = items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| item.to_string())
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("{key}: {rendered}");
            }
            Value::String(text) => println!("{key}: {text}"),
            _ => println!("{key}: {value}"),
        }
    }
}
