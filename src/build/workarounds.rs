use std::fs;

use anyhow::{Context, Result, anyhow};
use camino::Utf8PathBuf;

use crate::env::BuildEnv;
use crate::fsutil::{copy_dir_recursive, require_dir};

pub(crate) fn prepare_msbuild_workarounds(env: &BuildEnv) -> Result<()> {
    generate_vc_targets_overlay(env)?;
    generate_force_import_before_cpp_props(env)?;
    generate_directory_build_targets(env)?;
    Ok(())
}

pub(crate) fn generate_vc_targets_overlay(env: &BuildEnv) -> Result<()> {
    let source = env.msvc_root.join("MSBuild/Microsoft/VC/v170");
    let dest = vc_targets_overlay_path(env);
    require_dir(&source, "VCTargetsPath source")?;

    if dest.exists() {
        fs::remove_dir_all(dest.as_std_path())
            .with_context(|| format!("failed to remove old overlay {dest}"))?;
    }
    copy_dir_recursive(&source, &dest)?;

    let masm_dest = dest.join("BuildCustomizations/masm.targets");
    let masm_src = env.helper_root.join("msbuild/masm-wine.targets");
    fs::copy(masm_src.as_std_path(), masm_dest.as_std_path())
        .with_context(|| format!("failed to install {masm_dest}"))?;
    Ok(())
}

pub(crate) fn generate_directory_build_targets(env: &BuildEnv) -> Result<()> {
    let source = env
        .helper_root
        .join("msbuild/Directory.Build.targets.template");
    let dest = directory_build_targets_path(env);
    let parent = dest
        .parent()
        .ok_or_else(|| anyhow!("Directory.Build.targets path has no parent: {dest}"))?;
    fs::create_dir_all(parent.as_std_path())
        .with_context(|| format!("failed to create {parent}"))?;
    fs::copy(source.as_std_path(), dest.as_std_path())
        .with_context(|| format!("failed to generate {dest}"))?;
    Ok(())
}

pub(crate) fn generate_force_import_before_cpp_props(env: &BuildEnv) -> Result<()> {
    let source = env
        .helper_root
        .join("msbuild/ForceImportBeforeCppProps.props.template");
    let dest = force_import_before_cpp_props_path(env);
    let parent = dest
        .parent()
        .ok_or_else(|| anyhow!("ForceImportBeforeCppProps path has no parent: {dest}"))?;
    fs::create_dir_all(parent.as_std_path())
        .with_context(|| format!("failed to create {parent}"))?;
    fs::copy(source.as_std_path(), dest.as_std_path())
        .with_context(|| format!("failed to generate {dest}"))?;
    Ok(())
}

pub(crate) fn vc_targets_overlay_path(env: &BuildEnv) -> Utf8PathBuf {
    env.out_dir.join("msbuild/v170-overlay")
}

pub(crate) fn directory_build_targets_path(env: &BuildEnv) -> Utf8PathBuf {
    env.out_dir.join("msbuild/Directory.Build.targets")
}

pub(crate) fn force_import_before_cpp_props_path(env: &BuildEnv) -> Utf8PathBuf {
    env.out_dir.join("msbuild/ForceImportBeforeCppProps.props")
}
