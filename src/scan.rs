use std::collections::BTreeSet;
use std::fs;

use anyhow::{Context, Result, anyhow, bail};
use camino::Utf8Path;

use crate::artifacts::{
    DEFAULT_DEVICE_NAME, DEFAULT_DRIVER_FILE_NAME, DEFAULT_DRIVER_SERVICE_NAME,
    DEFAULT_SDK_DLL_NAME, fixed_staged_artifacts,
};
use crate::cli::{ScanArgs, ScanCommand, ScanStageArgs};
use crate::config::Config;
use crate::env::StageEnv;
use crate::fsutil::{require_file, sha256_file};
use crate::manifest::{ManifestArtifact, StageManifest, read_stage_manifest};
use crate::validate::{
    validate_manifest_extension, validate_manifest_filename, validate_manifest_label,
};

pub(crate) fn run(env: &StageEnv, args: ScanArgs) -> Result<()> {
    match args.command {
        ScanCommand::Stage(args) => scan_stage(env, args),
    }
}

pub(crate) fn scan_stage(env: &StageEnv, args: ScanStageArgs) -> Result<()> {
    let stage_dir = env.stage_dir(args.config);
    let manifest_path = stage_dir.join("release-manifest.json");
    let manifest = read_stage_manifest(&manifest_path)?;
    validate_stage_manifest(&manifest, args.config, args.require_custom)?;
    validate_stage_artifacts(&stage_dir, &manifest)?;
    scan_stage_bytes(&stage_dir, &manifest, args.config, args.require_custom)?;
    println!(
        "ok: scanned {} manifest and staged bytes ({})",
        env.helper_root
            .join("out/stage")
            .join(args.config.as_str())
            .join("release-manifest.json"),
        args.config.as_str()
    );
    Ok(())
}

fn validate_stage_manifest(
    manifest: &StageManifest,
    config: Config,
    require_custom: bool,
) -> Result<()> {
    validate_manifest_filename("sdk_dll_name", &manifest.sdk_dll_name)?;
    validate_manifest_filename("driver_file_name", &manifest.driver_file_name)?;
    validate_manifest_label("driver_service_name", &manifest.driver_service_name)?;
    validate_manifest_label("device_name", &manifest.device_name)?;
    for artifact in &manifest.artifacts {
        validate_manifest_filename("artifact.name", &artifact.name)?;
    }
    if manifest.build_config != config.as_str() {
        bail!(
            "manifest build_config must be {:?}, found {:?}",
            config.as_str(),
            manifest.build_config
        );
    }
    validate_manifest_extension("sdk_dll_name", &manifest.sdk_dll_name, ".dll")?;
    validate_manifest_extension("driver_file_name", &manifest.driver_file_name, ".sys")?;
    if manifest
        .sdk_dll_name
        .eq_ignore_ascii_case(&manifest.driver_file_name)
    {
        bail!("manifest sdk_dll_name and driver_file_name must be distinct");
    }

    let expected_nt = format!("\\Device\\{}", manifest.device_name);
    let expected_dos = format!("\\DosDevices\\{}", manifest.device_name);
    let expected_user = format!("\\\\.\\{}", manifest.device_name);
    require_manifest_value("nt_device_name", &manifest.nt_device_name, &expected_nt)?;
    require_manifest_value("dos_device_name", &manifest.dos_device_name, &expected_dos)?;
    require_manifest_value(
        "user_device_path",
        &manifest.user_device_path,
        &expected_user,
    )?;

    if require_custom {
        reject_default_token("sdk_dll_name", &manifest.sdk_dll_name, DEFAULT_SDK_DLL_NAME)?;
        reject_default_token(
            "driver_file_name",
            &manifest.driver_file_name,
            DEFAULT_DRIVER_FILE_NAME,
        )?;
        reject_default_token(
            "driver_service_name",
            &manifest.driver_service_name,
            DEFAULT_DRIVER_SERVICE_NAME,
        )?;
        reject_default_token("device_name", &manifest.device_name, DEFAULT_DEVICE_NAME)?;
        for artifact in &manifest.artifacts {
            reject_default_token("artifact.name", &artifact.name, DEFAULT_SDK_DLL_NAME)?;
            reject_default_token("artifact.name", &artifact.name, DEFAULT_DRIVER_FILE_NAME)?;
        }
    }

    require_artifact_declared(manifest, &manifest.sdk_dll_name)?;
    require_artifact_declared(manifest, &manifest.driver_file_name)?;
    for artifact in fixed_staged_artifacts(config) {
        require_artifact_declared(manifest, artifact)?;
    }
    Ok(())
}

fn require_manifest_value(label: &str, actual: &str, expected: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        bail!("manifest {label} must be {expected:?}, found {actual:?}")
    }
}

fn reject_default_value(label: &str, actual: &str, default: &str) -> Result<()> {
    if actual.eq_ignore_ascii_case(default) {
        bail!("manifest {label} must not use default value {default:?}");
    }
    Ok(())
}

fn reject_default_token(label: &str, actual: &str, default: &str) -> Result<()> {
    reject_default_value(label, actual, default)?;
    let token = Utf8Path::new(default)
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .unwrap_or(default);
    if actual
        .to_ascii_lowercase()
        .contains(&token.to_ascii_lowercase())
    {
        bail!("manifest {label} must not contain default token {token:?}: {actual:?}");
    }
    Ok(())
}

fn require_artifact_declared(manifest: &StageManifest, name: &str) -> Result<()> {
    if manifest
        .artifacts
        .iter()
        .any(|artifact| artifact.name == name)
    {
        Ok(())
    } else {
        bail!("manifest artifacts must include {name:?}")
    }
}

fn validate_stage_artifacts(stage_dir: &Utf8Path, manifest: &StageManifest) -> Result<()> {
    let mut declared = BTreeSet::new();
    for artifact in &manifest.artifacts {
        validate_manifest_artifact(stage_dir, artifact, &mut declared)?;
    }
    validate_stage_dir_coverage(stage_dir, &declared)?;
    Ok(())
}

fn validate_manifest_artifact(
    stage_dir: &Utf8Path,
    artifact: &ManifestArtifact,
    declared: &mut BTreeSet<String>,
) -> Result<()> {
    validate_manifest_filename("artifact.name", &artifact.name)?;
    let normalized_name = artifact.name.to_ascii_lowercase();
    if normalized_name == "release-manifest.json" {
        bail!("manifest must not declare release-manifest.json as an artifact");
    }
    if !declared.insert(normalized_name) {
        bail!("manifest artifact listed more than once: {}", artifact.name);
    }
    if !is_sha256_hex(&artifact.sha256) {
        bail!(
            "manifest artifact {} sha256 must be exactly 64 ASCII hex chars",
            artifact.name
        );
    }

    let path = stage_dir.join(&artifact.name);
    require_file(&path, "staged artifact")?;
    let metadata = fs::symlink_metadata(path.as_std_path())
        .with_context(|| format!("failed to stat staged artifact {path}"))?;
    if !metadata.file_type().is_file() {
        bail!("staged artifact {} must be a regular file", artifact.name);
    }
    if metadata.len() != artifact.size_bytes {
        bail!(
            "staged artifact {} size mismatch: manifest {}, actual {}",
            artifact.name,
            artifact.size_bytes,
            metadata.len()
        );
    }
    let actual_sha256 = sha256_file(&path)?;
    if actual_sha256 != artifact.sha256 {
        bail!(
            "staged artifact {} sha256 mismatch: manifest {}, actual {}",
            artifact.name,
            artifact.sha256,
            actual_sha256
        );
    }
    Ok(())
}

fn validate_stage_dir_coverage(stage_dir: &Utf8Path, declared: &BTreeSet<String>) -> Result<()> {
    for entry in fs::read_dir(stage_dir.as_std_path())
        .with_context(|| format!("failed to read stage dir {stage_dir}"))?
    {
        let entry =
            entry.with_context(|| format!("failed to read stage dir entry in {stage_dir}"))?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| anyhow!("non-UTF-8 staged artifact name: {:?}", entry.path()))?;
        if name == "release-manifest.json" || declared.contains(&name.to_ascii_lowercase()) {
            continue;
        }
        bail!("staged artifact missing from manifest: {name}");
    }
    Ok(())
}

fn scan_stage_bytes(
    stage_dir: &Utf8Path,
    manifest: &StageManifest,
    config: Config,
    require_custom: bool,
) -> Result<()> {
    let sdk_path = stage_dir.join(&manifest.sdk_dll_name);
    let driver_path = stage_dir.join(&manifest.driver_file_name);
    let sdk = fs::read(sdk_path.as_std_path())
        .with_context(|| format!("failed to read staged SDK DLL {sdk_path}"))?;
    let driver = fs::read(driver_path.as_std_path())
        .with_context(|| format!("failed to read staged driver SYS {driver_path}"))?;

    require_bytes(
        &sdk,
        manifest.driver_file_name.as_bytes(),
        "SDK DLL",
        "driver_file_name",
    )?;
    require_bytes(
        &sdk,
        manifest.driver_service_name.as_bytes(),
        "SDK DLL",
        "driver_service_name",
    )?;
    require_bytes(
        &sdk,
        manifest.user_device_path.as_bytes(),
        "SDK DLL",
        "user_device_path",
    )?;

    require_bytes(
        &driver,
        &utf16le(&manifest.nt_device_name),
        "driver SYS",
        "nt_device_name UTF-16LE",
    )?;
    require_bytes(
        &driver,
        &utf16le(&manifest.dos_device_name),
        "driver SYS",
        "dos_device_name UTF-16LE",
    )?;

    if require_custom {
        reject_bytes(
            &sdk,
            DEFAULT_DRIVER_FILE_NAME.as_bytes(),
            "SDK DLL",
            DEFAULT_DRIVER_FILE_NAME,
        )?;
        reject_bytes(
            &sdk,
            DEFAULT_DRIVER_SERVICE_NAME.as_bytes(),
            "SDK DLL",
            DEFAULT_DRIVER_SERVICE_NAME,
        )?;
        reject_bytes(
            &sdk,
            b"\\\\.\\HyperDbgDebuggerDevice",
            "SDK DLL",
            "\\\\.\\HyperDbgDebuggerDevice",
        )?;
        reject_bytes(
            &driver,
            &utf16le("\\Device\\HyperDbgDebuggerDevice"),
            "driver SYS",
            "\\Device\\HyperDbgDebuggerDevice UTF-16LE",
        )?;
        reject_bytes(
            &driver,
            &utf16le("\\DosDevices\\HyperDbgDebuggerDevice"),
            "driver SYS",
            "\\DosDevices\\HyperDbgDebuggerDevice UTF-16LE",
        )?;
        require_custom_consumer_bytes(stage_dir, manifest, config)?;
    }

    Ok(())
}

fn require_custom_consumer_bytes(
    stage_dir: &Utf8Path,
    manifest: &StageManifest,
    config: Config,
) -> Result<()> {
    for consumer in fixed_consumer_artifacts(config) {
        let path = stage_dir.join(consumer);
        let bytes = fs::read(path.as_std_path())
            .with_context(|| format!("failed to read staged consumer EXE {path}"))?;
        require_bytes(
            &bytes,
            manifest.sdk_dll_name.as_bytes(),
            consumer,
            "sdk_dll_name",
        )?;
        reject_bytes(
            &bytes,
            DEFAULT_SDK_DLL_NAME.as_bytes(),
            consumer,
            DEFAULT_SDK_DLL_NAME,
        )?;
    }
    Ok(())
}

fn fixed_consumer_artifacts(config: Config) -> &'static [&'static str] {
    match config {
        Config::Debug => &["hyperdbg-cli.exe", "hyperdbg-test.exe"],
        Config::Release => &["hyperdbg-cli.exe"],
    }
}

fn require_bytes(haystack: &[u8], needle: &[u8], artifact: &str, label: &str) -> Result<()> {
    if contains_bytes(haystack, needle) {
        Ok(())
    } else {
        bail!("{artifact} does not contain expected {label} bytes")
    }
}

fn reject_bytes(haystack: &[u8], needle: &[u8], artifact: &str, label: &str) -> Result<()> {
    if contains_bytes(haystack, needle) {
        bail!("{artifact} contains forbidden default {label} bytes")
    }
    Ok(())
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn utf16le(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use std::env;

    use camino::Utf8PathBuf;

    use super::*;
    use crate::artifacts::FIXED_STAGED_ARTIFACTS;
    use crate::fsutil::run_id;

    fn custom_stage_manifest() -> StageManifest {
        let mut artifacts = vec![
            ManifestArtifact {
                name: "ExampleSdk.dll".to_string(),
                size_bytes: 3,
                sha256: "unused".to_string(),
            },
            ManifestArtifact {
                name: "ExampleDriver.sys".to_string(),
                size_bytes: 3,
                sha256: "unused".to_string(),
            },
        ];
        artifacts.extend(FIXED_STAGED_ARTIFACTS.iter().map(|name| ManifestArtifact {
            name: (*name).to_string(),
            size_bytes: 3,
            sha256: "unused".to_string(),
        }));

        StageManifest {
            sdk_dll_name: "ExampleSdk.dll".to_string(),
            driver_file_name: "ExampleDriver.sys".to_string(),
            driver_service_name: "ExampleService".to_string(),
            device_name: "ExampleDevice".to_string(),
            nt_device_name: "\\Device\\ExampleDevice".to_string(),
            dos_device_name: "\\DosDevices\\ExampleDevice".to_string(),
            user_device_path: "\\\\.\\ExampleDevice".to_string(),
            build_config: "debug".to_string(),
            artifacts,
        }
    }

    fn release_stage_manifest() -> StageManifest {
        let mut manifest = custom_stage_manifest();
        manifest.build_config = "release".to_string();
        manifest
            .artifacts
            .retain(|artifact| artifact.name != "hyperdbg-test.exe");
        manifest
    }

    fn write_manifest_files(stage_dir: &Utf8Path, manifest: &mut StageManifest) {
        for artifact in &mut manifest.artifacts {
            let path = stage_dir.join(&artifact.name);
            fs::write(path.as_std_path(), b"abc").unwrap();
            artifact.size_bytes = 3;
            artifact.sha256 = sha256_file(&path).unwrap();
        }
    }

    fn write_custom_scan_bytes(stage_dir: &Utf8Path) {
        fs::write(
            stage_dir.join("ExampleSdk.dll").as_std_path(),
            b"ExampleDriver.sys\0ExampleService\0\\\\.\\ExampleDevice\0",
        )
        .unwrap();
        let mut driver_bytes = utf16le("\\Device\\ExampleDevice");
        driver_bytes.extend(utf16le("\\DosDevices\\ExampleDevice"));
        fs::write(
            stage_dir.join("ExampleDriver.sys").as_std_path(),
            driver_bytes,
        )
        .unwrap();
        fs::write(
            stage_dir.join("hyperdbg-cli.exe").as_std_path(),
            b"ExampleSdk.dll",
        )
        .unwrap();
        fs::write(
            stage_dir.join("hyperdbg-test.exe").as_std_path(),
            b"ExampleSdk.dll",
        )
        .unwrap();
    }

    #[test]
    fn scan_manifest_accepts_custom_contract() {
        let manifest = custom_stage_manifest();

        validate_stage_manifest(&manifest, Config::Debug, true).unwrap();
    }

    #[test]
    fn scan_manifest_rejects_wrong_config() {
        let mut manifest = custom_stage_manifest();
        manifest.build_config = "release".to_string();
        assert!(validate_stage_manifest(&manifest, Config::Debug, false).is_err());
    }

    #[test]
    fn scan_manifest_rejects_bad_device_derivations() {
        let mut manifest = custom_stage_manifest();
        manifest.user_device_path = "\\\\.\\OtherDevice".to_string();

        assert!(validate_stage_manifest(&manifest, Config::Debug, false).is_err());
    }

    #[test]
    fn scan_manifest_rejects_path_components() {
        let mut manifest = custom_stage_manifest();
        manifest.sdk_dll_name = "..\\ExampleSdk.dll".to_string();
        assert!(validate_stage_manifest(&manifest, Config::Debug, false).is_err());

        let mut manifest = custom_stage_manifest();
        manifest.artifacts[0].name = "../ExampleSdk.dll".to_string();
        assert!(validate_stage_manifest(&manifest, Config::Debug, false).is_err());
    }

    #[test]
    fn scan_manifest_rejects_bad_sdk_driver_shape() {
        let mut manifest = custom_stage_manifest();
        manifest.sdk_dll_name = "ExampleSdk.bin".to_string();
        assert!(validate_stage_manifest(&manifest, Config::Debug, false).is_err());

        let mut manifest = custom_stage_manifest();
        manifest.driver_file_name = "ExampleDriver.bin".to_string();
        assert!(validate_stage_manifest(&manifest, Config::Debug, false).is_err());

        let mut manifest = custom_stage_manifest();
        manifest.driver_file_name = "ExampleSdk.dll".to_string();
        assert!(validate_stage_manifest(&manifest, Config::Debug, false).is_err());
    }

    #[test]
    fn scan_manifest_rejects_missing_fixed_artifact() {
        let mut manifest = custom_stage_manifest();
        manifest
            .artifacts
            .retain(|artifact| artifact.name != FIXED_STAGED_ARTIFACTS[0]);

        assert!(validate_stage_manifest(&manifest, Config::Debug, false).is_err());
    }

    #[test]
    fn scan_manifest_requires_hyperdbg_test_for_debug_only() {
        let mut debug_manifest = custom_stage_manifest();
        debug_manifest
            .artifacts
            .retain(|artifact| artifact.name != "hyperdbg-test.exe");
        assert!(validate_stage_manifest(&debug_manifest, Config::Debug, false).is_err());

        let release_manifest = release_stage_manifest();
        validate_stage_manifest(&release_manifest, Config::Release, false).unwrap();
    }

    #[test]
    fn scan_manifest_require_custom_rejects_defaults() {
        let mut manifest = custom_stage_manifest();
        manifest.driver_service_name = DEFAULT_DRIVER_SERVICE_NAME.to_string();

        assert!(validate_stage_manifest(&manifest, Config::Debug, true).is_err());

        let mut manifest = custom_stage_manifest();
        manifest.driver_service_name = "Example-hyperkd".to_string();

        assert!(validate_stage_manifest(&manifest, Config::Debug, true).is_err());
    }

    #[test]
    fn scan_artifacts_validate_size_and_sha256() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-scan-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();

        let mut manifest = custom_stage_manifest();
        write_manifest_files(&stage_dir, &mut manifest);

        validate_stage_artifacts(&stage_dir, &manifest).unwrap();

        manifest.artifacts[0].size_bytes = 4;
        assert!(validate_stage_artifacts(&stage_dir, &manifest).is_err());

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }

    #[test]
    fn scan_artifacts_rejects_unmanifested_files_and_duplicate_names() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-coverage-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();

        let mut manifest = custom_stage_manifest();
        write_manifest_files(&stage_dir, &mut manifest);
        fs::write(stage_dir.join("Rogue.dll").as_std_path(), b"rogue").unwrap();

        assert!(validate_stage_artifacts(&stage_dir, &manifest).is_err());
        fs::remove_file(stage_dir.join("Rogue.dll").as_std_path()).unwrap();

        manifest.artifacts.push(ManifestArtifact {
            name: "ExampleSdk.dll".to_string(),
            size_bytes: 3,
            sha256: sha256_file(&stage_dir.join("ExampleSdk.dll")).unwrap(),
        });
        assert!(validate_stage_artifacts(&stage_dir, &manifest).is_err());

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }

    #[test]
    fn scan_artifacts_rejects_case_insensitive_duplicate_names() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-case-dupe-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();

        let mut manifest = custom_stage_manifest();
        write_manifest_files(&stage_dir, &mut manifest);
        manifest.artifacts.push(ManifestArtifact {
            name: "examplesdk.dll".to_string(),
            size_bytes: 3,
            sha256: sha256_file(&stage_dir.join("ExampleSdk.dll")).unwrap(),
        });

        assert!(validate_stage_artifacts(&stage_dir, &manifest).is_err());

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }

    #[test]
    fn scan_artifacts_rejects_manifest_as_artifact() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-manifest-artifact-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();

        let mut manifest = custom_stage_manifest();
        write_manifest_files(&stage_dir, &mut manifest);
        fs::write(
            stage_dir.join("release-manifest.json").as_std_path(),
            b"abc",
        )
        .unwrap();
        manifest.artifacts.push(ManifestArtifact {
            name: "release-manifest.json".to_string(),
            size_bytes: 3,
            sha256: sha256_file(&stage_dir.join("release-manifest.json")).unwrap(),
        });

        assert!(validate_stage_artifacts(&stage_dir, &manifest).is_err());

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }

    #[test]
    fn scan_artifacts_rejects_bad_sha256_shape_before_compare() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-bad-sha-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();

        let mut manifest = custom_stage_manifest();
        write_manifest_files(&stage_dir, &mut manifest);
        manifest.artifacts[0].sha256 = "not-a-sha".to_string();

        let err = validate_stage_artifacts(&stage_dir, &manifest).unwrap_err();
        assert!(err.to_string().contains("64 ASCII hex"));

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }

    #[test]
    fn scan_artifacts_rejects_non_regular_entries() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-nonfile-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();
        let mut manifest = custom_stage_manifest();
        write_manifest_files(&stage_dir, &mut manifest);
        fs::remove_file(stage_dir.join("ExampleSdk.dll").as_std_path()).unwrap();
        fs::create_dir(stage_dir.join("ExampleSdk.dll").as_std_path()).unwrap();

        assert!(validate_stage_artifacts(&stage_dir, &manifest).is_err());

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }

    #[test]
    fn scan_bytes_require_expected_custom_values() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-byte-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();
        write_custom_scan_bytes(&stage_dir);

        let manifest = custom_stage_manifest();
        scan_stage_bytes(&stage_dir, &manifest, Config::Debug, true).unwrap();

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }

    #[test]
    fn scan_bytes_require_custom_rejects_default_values() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-default-byte-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();
        fs::write(
            stage_dir.join("ExampleSdk.dll").as_std_path(),
            b"ExampleDriver.sys\0ExampleService\0\\\\.\\ExampleDevice\0hyperkd.sys\0",
        )
        .unwrap();
        let mut driver_bytes = utf16le("\\Device\\ExampleDevice");
        driver_bytes.extend(utf16le("\\DosDevices\\ExampleDevice"));
        fs::write(
            stage_dir.join("ExampleDriver.sys").as_std_path(),
            driver_bytes,
        )
        .unwrap();
        fs::write(
            stage_dir.join("hyperdbg-cli.exe").as_std_path(),
            b"ExampleSdk.dll",
        )
        .unwrap();
        fs::write(
            stage_dir.join("hyperdbg-test.exe").as_std_path(),
            b"ExampleSdk.dll",
        )
        .unwrap();

        let manifest = custom_stage_manifest();
        assert!(scan_stage_bytes(&stage_dir, &manifest, Config::Debug, true).is_err());

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }

    #[test]
    fn scan_bytes_require_custom_proves_consumer_sdk_dll_name() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-consumer-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();
        write_custom_scan_bytes(&stage_dir);
        fs::write(stage_dir.join("hyperdbg-test.exe").as_std_path(), b"other").unwrap();

        let manifest = custom_stage_manifest();
        assert!(scan_stage_bytes(&stage_dir, &manifest, Config::Debug, true).is_err());

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }

    #[test]
    fn release_scan_bytes_require_custom_does_not_require_hyperdbg_test() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-release-consumer-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();
        write_custom_scan_bytes(&stage_dir);
        fs::remove_file(stage_dir.join("hyperdbg-test.exe").as_std_path()).unwrap();

        let manifest = release_stage_manifest();
        scan_stage_bytes(&stage_dir, &manifest, Config::Release, true).unwrap();

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }

    #[test]
    fn scan_bytes_require_custom_rejects_default_consumer_sdk_dll_name() {
        let unique = run_id().unwrap();
        let stage_dir = Utf8PathBuf::from_path_buf(env::temp_dir())
            .unwrap()
            .join(format!("hyperdbg-build-consumer-default-test-{unique}"));
        fs::create_dir_all(stage_dir.as_std_path()).unwrap();
        write_custom_scan_bytes(&stage_dir);
        fs::write(
            stage_dir.join("hyperdbg-cli.exe").as_std_path(),
            b"ExampleSdk.dll\0libhyperdbg.dll",
        )
        .unwrap();

        let manifest = custom_stage_manifest();
        assert!(scan_stage_bytes(&stage_dir, &manifest, Config::Debug, true).is_err());

        fs::remove_dir_all(stage_dir.as_std_path()).unwrap();
    }
}
