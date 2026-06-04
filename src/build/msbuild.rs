use anyhow::{Context, Result};
use camino::Utf8Path;

use crate::artifacts::ArtifactNames;
use crate::build::ensure_allowed_target;
use crate::build::workarounds::{directory_build_targets_path, vc_targets_overlay_path};
use crate::config::{Config, PLATFORM};
use crate::env::BuildEnv;
use crate::fsutil::{wine_dir_path, wine_path};
use crate::process::run_logged;

pub(crate) fn build_msbuild_target(
    env: &BuildEnv,
    log_dir: &Utf8Path,
    target: &str,
    config: Config,
    workarounds: bool,
    _artifacts: &ArtifactNames,
) -> Result<()> {
    ensure_allowed_target(target)?;
    let mut command = env.command(&env.msbuild)?;
    command
        .arg("/nologo")
        .arg(format!("/p:Configuration={}", config.as_str()))
        .arg(format!("/p:Platform={PLATFORM}"))
        .arg(env.solution.as_str())
        .arg(format!("/t:{target}"));

    if workarounds {
        command.arg(format!(
            "/p:DirectoryBuildTargetsPath={}",
            wine_path(&directory_build_targets_path(env))?
        ));
        command.arg(format!(
            "/p:VCTargetsPath={}",
            wine_dir_path(&vc_targets_overlay_path(env))?
        ));
    }

    let log = log_dir.join(format!("{target}.log"));
    println!("building {target} ({})", config.as_str());
    run_logged(command, &log).with_context(|| format!("msbuild target {target} failed; see {log}"))
}
