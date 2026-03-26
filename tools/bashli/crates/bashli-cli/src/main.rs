mod input;
mod output;

use bashli_engine::EngineBuilder;
use clap::Parser;
use std::process;

#[derive(Parser)]
#[command(name = "bashli", version, about = "Structured bash execution engine for AI agents")]
struct Cli {
    /// Inline JSON task specification
    spec: Option<String>,

    /// Read task spec from a file
    #[arg(short = 'f', long = "file")]
    file: Option<String>,

    /// Output verbosity
    #[arg(short = 'v', long = "verbosity", default_value = "normal")]
    verbosity: String,

    /// Pretty-print JSON output
    #[arg(short = 'p', long = "pretty")]
    pretty: bool,

    /// Global timeout in milliseconds
    #[arg(short = 't', long = "timeout", default_value = "300000")]
    timeout: u64,

    /// Shell to use
    #[arg(long = "shell")]
    shell: Option<String>,

    /// Disable write steps and redirects
    #[arg(long = "read-only")]
    read_only: bool,

    /// Restrict write targets to a glob pattern
    #[arg(long = "allowed-paths")]
    allowed_paths: Option<Vec<String>>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Parse input
    let spec = match input::parse_input(&cli.spec, &cli.file) {
        Ok(spec) => spec,
        Err(e) => {
            let err = serde_json::json!({
                "ok": false,
                "error": {
                    "kind": "validation_error",
                    "message": e.to_string()
                }
            });
            eprintln!("{}", serde_json::to_string(&err).unwrap());
            process::exit(2);
        }
    };

    // Build engine
    let mut builder = EngineBuilder::new()
        .timeout(std::time::Duration::from_millis(cli.timeout))
        .read_only(cli.read_only);

    if let Some(ref shell) = cli.shell {
        builder = builder.shell(vec![shell.clone(), "-c".into()]);
    }

    if let Some(ref paths) = cli.allowed_paths {
        builder = builder.allowed_paths(paths.clone());
    }

    let engine = builder.build();

    // Run
    let result = engine.run(spec).await;
    let exit_code = if result.ok { 0 } else { 1 };

    // Output
    let json = if cli.pretty {
        serde_json::to_string_pretty(&result)
    } else {
        serde_json::to_string(&result)
    };

    match json {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("{{\"ok\":false,\"error\":{{\"message\":\"{e}\"}}}}");
            process::exit(2);
        }
    }

    process::exit(exit_code);
}
