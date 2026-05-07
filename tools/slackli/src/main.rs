use clap::Parser;
use slackli::app;
use slackli::cli::Cli;

fn main() {
    let cli = Cli::parse();

    match app::run(cli) {
        Ok(output) => println!("{output}"),
        Err(err) => {
            eprintln!("failed to serialize output: {err}");
            std::process::exit(1);
        }
    }
}
