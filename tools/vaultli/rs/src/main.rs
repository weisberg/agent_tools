use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde_json::{json, Map, Value};
use vaultli::context::assemble_context;
use vaultli::error::VaultliError;
use vaultli::federation::federated_search;
use vaultli::gitinfo::git_info;
use vaultli::id::make_id;
use vaultli::index::{build_index, load_index_records};
use vaultli::infer::infer_frontmatter;
use vaultli::metadata::{refresh_metadata, set_metadata_field, unset_metadata_field};
use vaultli::paths::find_root;
use vaultli::scaffold::{add_file, ingest_path, init_vault, scaffold_file};
use vaultli::search::{cat_record, resolve_record, search_records, show_record};
use vaultli::validate::validate_vault;

#[derive(Parser)]
#[command(
    name = "vaultli",
    version,
    about = "Rust implementation of the vaultli knowledge-vault CLI"
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
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        sort: Option<String>,
        #[arg(long, default_value = "asc")]
        order: String,
        #[arg(long)]
        explain: bool,
        #[arg(long)]
        semantic: bool,
    },
    FederatedSearch {
        query: Option<String>,
        #[arg(long = "vault", required = true)]
        vaults: Vec<PathBuf>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        per_vault_limit: Option<usize>,
        #[arg(long)]
        sort: Option<String>,
        #[arg(long, default_value = "asc")]
        order: String,
        #[arg(long)]
        semantic: bool,
        #[arg(long)]
        explain: bool,
    },
    Show {
        id: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Resolve {
        id: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        body: bool,
        #[arg(long)]
        source: bool,
    },
    Cat {
        id: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        source: bool,
    },
    Context {
        query: Option<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long = "id")]
        ids: Vec<String>,
        #[arg(long)]
        token_budget: Option<i64>,
        #[arg(long)]
        related: bool,
        #[arg(long)]
        no_dependencies: bool,
        #[arg(long)]
        limit: Option<usize>,
    },
    GitInfo {
        target: Option<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Scaffold {
        file: PathBuf,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Ingest {
        path: PathBuf,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        index: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        include: Vec<String>,
        #[arg(long)]
        exclude: Vec<String>,
    },
    Set {
        target: String,
        field: String,
        value: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        index: bool,
    },
    Unset {
        target: String,
        field: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        index: bool,
    },
    Refresh {
        target: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long = "field")]
        fields: Vec<String>,
        #[arg(long)]
        index: bool,
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
        Commands::Search {
            query,
            root,
            jq,
            category,
            status,
            domain,
            scope,
            tags,
            limit,
            sort,
            order,
            explain,
            semantic,
        } => {
            let result = search_records(
                &root,
                query.as_deref(),
                jq.as_deref(),
                category.as_deref(),
                status.as_deref(),
                domain.as_deref(),
                scope.as_deref(),
                &tags,
                limit,
                sort.as_deref(),
                &order,
                explain,
                semantic,
            )
            .map_err(|error| (error, as_json))?;
            emit_result(json!({ "results": result, "total": result.len() }), as_json);
        }
        Commands::FederatedSearch {
            query,
            vaults,
            limit,
            per_vault_limit,
            sort,
            order,
            semantic,
            explain,
        } => {
            let result = federated_search(
                &vaults,
                query.as_deref(),
                limit,
                per_vault_limit,
                semantic,
                explain,
                sort.as_deref(),
                &order,
            )
            .map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Show { id, root } => {
            let result = show_record(&root, &id).map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Resolve {
            id,
            root,
            body,
            source,
        } => {
            let result =
                resolve_record(&root, &id, body, source).map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Cat { id, root, source } => {
            let result = cat_record(&root, &id, source).map_err(|error| (error, as_json))?;
            if as_json {
                emit_result(Value::Object(result), as_json);
            } else {
                let content = result
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if content.ends_with('\n') {
                    print!("{content}");
                } else {
                    println!("{content}");
                }
            }
        }
        Commands::Context {
            query,
            root,
            ids,
            token_budget,
            related,
            no_dependencies,
            limit,
        } => {
            let result = assemble_context(
                &root,
                query.as_deref(),
                &ids,
                token_budget,
                related,
                !no_dependencies,
                limit,
            )
            .map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::GitInfo { target, root } => {
            let result = git_info(&root, target.as_deref()).map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Scaffold { file, root } => {
            let result = scaffold_file(&root, &file).map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Ingest {
            path,
            root,
            index,
            dry_run,
            include,
            exclude,
        } => {
            let result = ingest_path(&root, &path, index, dry_run, &include, &exclude)
                .map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Set {
            target,
            field,
            value,
            root,
            index,
        } => {
            let result = set_metadata_field(&root, &target, &field, &value, index)
                .map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Unset {
            target,
            field,
            root,
            index,
        } => {
            let result = unset_metadata_field(&root, &target, &field, index)
                .map_err(|error| (error, as_json))?;
            emit_result(Value::Object(result), as_json);
        }
        Commands::Refresh {
            target,
            root,
            fields,
            index,
        } => {
            let result = refresh_metadata(&root, &target, &fields, index)
                .map_err(|error| (error, as_json))?;
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
