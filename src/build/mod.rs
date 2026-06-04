use std::fs;

use anyhow::{Context, Result, bail};
use camino::Utf8Path;

use crate::artifacts::ArtifactNames;
use crate::cli::{BuildArgs, BuildCommand};
use crate::config::Config;
use crate::env::BuildEnv;
use crate::fsutil::run_id;

pub(crate) mod asm;
mod msbuild;
mod stage;
mod workarounds;

use asm::preassemble_asm;
use msbuild::build_msbuild_target;
use stage::{copy_script_engine_scripts, stage_artifacts};
use workarounds::prepare_msbuild_workarounds;

pub(crate) const ALLOWED_TARGETS: &[&str] = &[
    "pdbex",
    "symbol-parser",
    "script-engine",
    "hyperlog",
    "kdserial",
    "hypertrace",
    "hyperevade",
    "hyperhv",
    "hyperkd",
    "libhyperdbg",
    "hyperdbg-test",
    "hyperdbg-cli",
];

pub(crate) const LANE_TARGETS: &[&str] = &[
    "pdbex",
    "symbol-parser",
    "script-engine",
    "hyperlog",
    "kdserial",
    "hypertrace",
    "hyperevade",
    "hyperhv",
    "hyperkd",
    "libhyperdbg",
    "hyperdbg-test",
    "hyperdbg-cli",
];

pub(crate) fn lane_targets(config: Config) -> impl Iterator<Item = &'static str> {
    LANE_TARGETS
        .iter()
        .copied()
        .filter(move |target| config == Config::Debug || *target != "hyperdbg-test")
}

pub(crate) fn run(env: &BuildEnv, args: BuildArgs) -> Result<()> {
    env.require_tools()?;
    let run_id = run_id()?;
    let log_dir = env.log_dir(&run_id);
    fs::create_dir_all(log_dir.as_std_path())
        .with_context(|| format!("failed to create {log_dir}"))?;
    println!("run: {run_id}");
    println!("logs: {log_dir}");

    match args.command {
        BuildCommand::Hyperevade(config) => {
            let artifacts = ArtifactNames::from_args(&config.artifacts)?;
            build_target(env, &log_dir, "hyperevade", config.config, &artifacts)
        }
        BuildCommand::Target(target) => {
            let artifacts = ArtifactNames::from_args(&target.artifacts)?;
            build_target(env, &log_dir, &target.name, target.config, &artifacts)
        }
        BuildCommand::Lane(config) => {
            let artifacts = ArtifactNames::from_args(&config.artifacts)?;
            build_lane(env, &log_dir, config.config, &artifacts)
        }
    }
}

pub(crate) fn build_lane(
    env: &BuildEnv,
    log_dir: &Utf8Path,
    config: Config,
    artifacts: &ArtifactNames,
) -> Result<()> {
    prepare_msbuild_workarounds(env)?;
    preassemble_asm(env, log_dir, config, None)?;

    for target in lane_targets(config) {
        build_msbuild_target(env, log_dir, target, config, true, artifacts)?;
        if target == "script-engine" {
            copy_script_engine_scripts(env, config)?;
        }
    }

    stage_artifacts(env, config, artifacts)?;
    Ok(())
}

pub(crate) fn build_target(
    env: &BuildEnv,
    log_dir: &Utf8Path,
    target: &str,
    config: Config,
    artifacts: &ArtifactNames,
) -> Result<()> {
    ensure_allowed_target(target)?;
    let needs_workarounds = target_needs_workarounds(target);
    if needs_workarounds {
        prepare_msbuild_workarounds(env)?;
        if target_may_build_masm(target) {
            preassemble_asm(env, log_dir, config, None)?;
        }
    }

    build_msbuild_target(env, log_dir, target, config, needs_workarounds, artifacts)?;
    if target == "script-engine" {
        copy_script_engine_scripts(env, config)?;
    }
    Ok(())
}

pub(crate) fn ensure_allowed_target(target: &str) -> Result<()> {
    if ALLOWED_TARGETS.contains(&target) {
        Ok(())
    } else {
        bail!(
            "target {target:?} is not allowlisted; allowed targets: {}",
            ALLOWED_TARGETS.join(", ")
        )
    }
}

pub(super) fn target_needs_workarounds(target: &str) -> bool {
    target == "script-engine" || target_may_build_masm(target)
}

pub(super) fn target_may_build_masm(target: &str) -> bool {
    matches!(
        target,
        "hyperhv" | "hyperkd" | "libhyperdbg" | "hyperdbg-test" | "hyperdbg-cli"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_targets_include_hyperdbg_test_for_debug_only() {
        let debug_targets = lane_targets(Config::Debug).collect::<Vec<_>>();
        let release_targets = lane_targets(Config::Release).collect::<Vec<_>>();

        assert_eq!(debug_targets, LANE_TARGETS);
        assert!(ALLOWED_TARGETS.contains(&"hyperdbg-test"));
        assert!(debug_targets.contains(&"hyperdbg-test"));
        assert!(!release_targets.contains(&"hyperdbg-test"));
        assert!(release_targets.contains(&"hyperdbg-cli"));
    }
}
