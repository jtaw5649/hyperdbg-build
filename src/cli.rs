use clap::{Args, Parser, Subcommand};

use crate::config::Config;

#[derive(Parser)]
#[command(author, version, about)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    Env,
    Build(BuildArgs),
    Scan(ScanArgs),
}

#[derive(Args)]
pub(crate) struct BuildArgs {
    #[command(subcommand)]
    pub(crate) command: BuildCommand,
}

#[derive(Subcommand)]
pub(crate) enum BuildCommand {
    Hyperevade(BuildConfig),
    Target(TargetArgs),
    Lane(BuildConfig),
}

#[derive(Args)]
pub(crate) struct ScanArgs {
    #[command(subcommand)]
    pub(crate) command: ScanCommand,
}

#[derive(Subcommand)]
pub(crate) enum ScanCommand {
    Stage(ScanStageArgs),
}

#[derive(Args)]
pub(crate) struct ScanStageArgs {
    #[arg(long, value_enum, default_value_t = Config::Debug)]
    pub(crate) config: Config,
    #[arg(long)]
    pub(crate) require_custom: bool,
}

#[derive(Args, Clone)]
pub(crate) struct BuildConfig {
    #[arg(long, value_enum, default_value_t = Config::Debug)]
    pub(crate) config: Config,
    #[command(flatten)]
    pub(crate) artifacts: ArtifactArgs,
}

#[derive(Args, Clone, Default)]
pub(crate) struct ArtifactArgs {
    #[arg(long)]
    pub(crate) sdk_dll_name: Option<String>,
    #[arg(long)]
    pub(crate) driver_file_name: Option<String>,
    #[arg(long)]
    pub(crate) driver_service_name: Option<String>,
    #[arg(long)]
    pub(crate) device_name: Option<String>,
}

#[derive(Args)]
pub(crate) struct TargetArgs {
    pub(crate) name: String,
    #[arg(long, value_enum, default_value_t = Config::Debug)]
    pub(crate) config: Config,
    #[command(flatten)]
    pub(crate) artifacts: ArtifactArgs,
}
