#![forbid(unsafe_code)]

mod cli;
mod content;
mod context;
mod error;
mod notion;
mod output;
mod query;
mod resolve;
mod runner;
mod schema;
mod storage;
mod util;

use std::time::Instant;

use clap::Parser;

use cli::Cli;
use context::Context;
use output::{exit_error, exit_ok, OutputOptions};
use runner::run;
use schema::command_name;

fn main() {
    let cli = Cli::parse();
    let output = OutputOptions::from_cli(&cli);
    let ctx = match Context::from_cli(&cli) {
        Ok(ctx) => ctx,
        Err(error) => exit_error(error, "init", Instant::now(), output),
    };

    let command_name = command_name(&cli.command);
    let result = run(cli.command, &ctx);
    match result {
        Ok(value) => exit_ok(value, command_name, &ctx),
        Err(error) => exit_error(error, command_name, ctx.started_at, output),
    }
}
