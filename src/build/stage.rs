use std::fs;
use std::process::Command;

use anyhow::{Context, Result, bail};
use camino::Utf8Path;

use crate::artifacts::{ArtifactNames, staged_artifact_names};
use crate::config::Config;
use crate::env::BuildEnv;
use crate::fsutil::{copy_files_with_extension, require_dir, sha256_file};
use crate::manifest::{StagedArtifact, manifest_json};

pub(crate) fn copy_script_engine_scripts(env: &BuildEnv, config: Config) -> Result<()> {
    let source = env.repo_root.join("hyperdbg/script-engine/script");
    let dest = env
        .repo_root
        .join("hyperdbg/build/bin")
        .join(config.as_str())
        .join("script");
    require_dir(&source, "script-engine script source")?;
    copy_files_with_extension(&source, &dest, "ds")?;
    println!("copied script-engine .ds files to {dest}");
    Ok(())
}

pub(crate) fn stage_artifacts(
    env: &BuildEnv,
    config: Config,
    artifacts: &ArtifactNames,
) -> Result<()> {
    let bin_dir = env
        .repo_root
        .join("hyperdbg/build/bin")
        .join(config.as_str());
    let stage_dir = env.out_dir.join("stage").join(config.as_str());
    if stage_dir.exists() {
        fs::remove_dir_all(stage_dir.as_std_path())
            .with_context(|| format!("failed to remove old stage dir {stage_dir}"))?;
    }
    fs::create_dir_all(stage_dir.as_std_path())
        .with_context(|| format!("failed to create stage dir {stage_dir}"))?;

    let mut missing = Vec::new();
    for artifact in staged_artifact_names(config, artifacts) {
        let source = bin_dir.join(artifact);
        if source.exists() {
            let dest = stage_dir.join(artifact);
            fs::copy(source.as_std_path(), dest.as_std_path())
                .with_context(|| format!("failed to stage {artifact}"))?;
        } else {
            missing.push(artifact);
        }
    }

    if !missing.is_empty() {
        bail!(
            "stage failed; missing artifacts in {}: {}",
            bin_dir,
            missing.join(", ")
        );
    }

    write_manifest(env, &stage_dir, config, artifacts)?;
    println!("staged artifacts to {stage_dir}");
    Ok(())
}

pub(crate) fn write_manifest(
    env: &BuildEnv,
    stage_dir: &Utf8Path,
    config: Config,
    artifacts: &ArtifactNames,
) -> Result<()> {
    let manifest = stage_dir.join("release-manifest.json");
    let staged_artifacts =
        collect_staged_artifacts(stage_dir, staged_artifact_names(config, artifacts))?;
    fs::write(
        manifest.as_std_path(),
        manifest_json(artifacts, &source_commit(env), config, &staged_artifacts),
    )
    .with_context(|| format!("failed to write {manifest}"))?;
    Ok(())
}

pub(crate) fn collect_staged_artifacts(
    stage_dir: &Utf8Path,
    names: Vec<&str>,
) -> Result<Vec<StagedArtifact>> {
    let mut artifacts = Vec::new();
    for name in names {
        let path = stage_dir.join(name);
        let metadata = fs::metadata(path.as_std_path())
            .with_context(|| format!("failed to stat staged artifact {path}"))?;
        artifacts.push(StagedArtifact {
            name: name.to_string(),
            size_bytes: metadata.len(),
            sha256: sha256_file(&path)?,
        });
    }
    Ok(artifacts)
}

pub(crate) fn source_commit(env: &BuildEnv) -> String {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(env.repo_root.as_std_path())
        .output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;

    use camino::Utf8PathBuf;

    use super::*;
    use crate::fsutil::run_id;

    #[test]
    fn staged_artifact_metadata_includes_size_and_sha256() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();
        fs::write(stage_dir.join("artifact.bin").as_std_path(), b"abc").unwrap();

        let artifacts = collect_staged_artifacts(&stage_dir, vec!["artifact.bin"]).unwrap();

        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "artifact.bin");
        assert_eq!(artifacts[0].size_bytes, 3);
        assert_eq!(
            artifacts[0].sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }
}
