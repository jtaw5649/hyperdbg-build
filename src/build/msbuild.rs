use anyhow::{Context, Result};
use camino::Utf8Path;

use crate::artifacts::ArtifactNames;
use crate::build::ensure_allowed_target;
use crate::build::workarounds::{
    directory_build_targets_path, force_import_before_cpp_props_path, vc_targets_overlay_path,
};
use crate::config::{Config, PLATFORM};
use crate::env::BuildEnv;
use crate::fsutil::{wine_dir_path, wine_path};
use crate::process::run_logged;

const NUGET_SDK_TARGET_PLATFORM_VERSION: &str = "10.0.28000.0";

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
        .arg(format!(
            "/p:TargetPlatformVersion={NUGET_SDK_TARGET_PLATFORM_VERSION}"
        ))
        .arg(format!(
            "/p:WindowsTargetPlatformVersion={NUGET_SDK_TARGET_PLATFORM_VERSION}"
        ))
        .arg(format!(
            "/p:WDKBuildFolder={NUGET_SDK_TARGET_PLATFORM_VERSION}"
        ))
        .arg("/p:GenerateManifest=false")
        .arg("/p:EmbedManifest=false")
        .arg(env.solution.as_str())
        .arg(format!("/t:{target}"));

    if workarounds {
        let directory_build_targets = wine_path(&directory_build_targets_path(env))?;
        let force_import_before_cpp_props = wine_path(&force_import_before_cpp_props_path(env))?;
        let vc_targets_overlay = wine_dir_path(&vc_targets_overlay_path(env))?;

        command
            .env("DirectoryBuildTargetsPath", &directory_build_targets)
            .env("ForceImportBeforeCppProps", &force_import_before_cpp_props)
            .env("VCTargetsPath", &vc_targets_overlay)
            .arg(format!(
                "/p:DirectoryBuildTargetsPath={directory_build_targets}"
            ))
            .arg(format!(
                "/p:ForceImportBeforeCppProps={force_import_before_cpp_props}"
            ))
            .arg(format!("/p:VCTargetsPath={vc_targets_overlay}"));
    }

    let log = log_dir.join(format!("{target}.log"));
    println!("building {target} ({})", config.as_str());
    run_logged(command, &log).with_context(|| format!("msbuild target {target} failed; see {log}"))
}
