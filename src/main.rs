use anyhow::Result;
use clap::Parser;

mod artifacts;
mod build;
mod cli;
mod config;
mod env;
mod fsutil;
mod manifest;
mod process;
mod scan;
mod validate;

use cli::{Cli, Commands};
use env::{BuildEnv, StageEnv};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Env => env::print_env(&BuildEnv::detect()?),
        Commands::Build(args) => build::run(&BuildEnv::detect()?, args),
        Commands::Scan(args) => scan::run(&StageEnv::detect()?, args),
    }
}
