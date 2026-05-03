use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(name = "lira", version, about = "Local Jira for agents")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Init {
        #[arg(long)]
        dry_run: bool,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
}

#[derive(Subcommand, Debug)]
enum ProjectCommands {
    List,
    Create { key: String, name: String },
    Show { key: String },
}

#[derive(Serialize)]
struct Envelope<T: Serialize> {
    ok: bool,
    data: T,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init { dry_run }) => cmd_init(cli.json, dry_run),
        Some(Commands::Project { command }) => match command {
            ProjectCommands::List => cmd_project_list(cli.json),
            ProjectCommands::Create { key, name } => cmd_project_create(cli.json, &key, &name),
            ProjectCommands::Show { key } => cmd_project_show(cli.json, &key),
        },
        None => cmd_version(cli.json),
    }
}

fn cmd_version(json: bool) -> Result<()> {
    if json {
        print_json(Envelope {
            ok: true,
            data: serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }),
        })
    } else {
        println!("{}", env!("CARGO_PKG_VERSION"));
        Ok(())
    }
}

fn cmd_init(json: bool, dry_run: bool) -> Result<()> {
    let report = lira_store::init_workspace(dry_run)?;

    if json {
        print_json(Envelope {
            ok: true,
            data: serde_json::to_value(report)?,
        })
    } else {
        println!(
            "{} {}",
            if dry_run {
                "would initialize"
            } else {
                "initialized"
            },
            report.root.display()
        );
        Ok(())
    }
}

fn cmd_project_list(json: bool) -> Result<()> {
    let dir = lira_store::project_dir()?;
    let mut projects = Vec::<String>::new();
    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    projects.push(name.to_string());
                }
            }
        }
    }

    projects.sort();

    if json {
        print_json(Envelope {
            ok: true,
            data: serde_json::json!({ "projects": projects }),
        })
    } else {
        for p in projects {
            println!("{p}");
        }
        Ok(())
    }
}

fn cmd_project_create(json: bool, key: &str, name: &str) -> Result<()> {
    let project = lira_store::project_dir()?.join(key);
    std::fs::create_dir_all(&project)?;
    std::fs::create_dir_all(project.join("tickets/todo"))?;
    std::fs::create_dir_all(project.join("tickets/in-progress"))?;
    std::fs::create_dir_all(project.join("tickets/in-review"))?;
    std::fs::create_dir_all(project.join("tickets/done"))?;

    let project_yaml = serde_yaml::to_string(&serde_json::json!({
        "key": key,
        "name": name,
        "schema_version": 3,
    }))?;
    std::fs::write(project.join("project.yaml"), project_yaml)?;

    if json {
        print_json(Envelope {
            ok: true,
            data: serde_json::json!({ "project": key, "name": name }),
        })
    } else {
        println!("created project {key}");
        Ok(())
    }
}

fn cmd_project_show(json: bool, key: &str) -> Result<()> {
    let path = lira_store::project_dir()?.join(key).join("project.yaml");
    if !path.exists() {
        let err = lira_core::ErrorEnvelope::new(
            "E_PROJECT_NOT_FOUND",
            format!("Project '{key}' not found"),
        )
        .with_suggestion("lira project list --json", "list available projects");
        if json {
            print_json(err)?;
            return Ok(());
        }
        anyhow::bail!("Project '{key}' not found")
    }

    let body = std::fs::read_to_string(path)?;
    let value: serde_yaml::Value = serde_yaml::from_str(&body)?;
    if json {
        print_json(Envelope {
            ok: true,
            data: serde_json::to_value(value)?,
        })
    } else {
        println!("{body}");
        Ok(())
    }
}

fn print_json<T: Serialize>(value: T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
