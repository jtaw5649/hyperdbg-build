use std::fs;

use anyhow::{Context, Result};
use camino::Utf8Path;
use serde::Deserialize;

use crate::artifacts::ArtifactNames;
use crate::config::Config;

pub(crate) struct StagedArtifact {
    pub(crate) name: String,
    pub(crate) size_bytes: u64,
    pub(crate) sha256: String,
}

#[derive(Deserialize)]
pub(crate) struct StageManifest {
    pub(crate) sdk_dll_name: String,
    pub(crate) driver_file_name: String,
    pub(crate) driver_service_name: String,
    pub(crate) device_name: String,
    pub(crate) nt_device_name: String,
    pub(crate) dos_device_name: String,
    pub(crate) user_device_path: String,
    pub(crate) build_config: String,
    pub(crate) artifacts: Vec<ManifestArtifact>,
}

#[derive(Deserialize)]
pub(crate) struct ManifestArtifact {
    pub(crate) name: String,
    pub(crate) size_bytes: u64,
    pub(crate) sha256: String,
}

pub(crate) fn read_stage_manifest(path: &Utf8Path) -> Result<StageManifest> {
    let content = fs::read_to_string(path.as_std_path())
        .with_context(|| format!("failed to read stage manifest {path}"))?;
    serde_json::from_str(&content).with_context(|| format!("failed to parse stage manifest {path}"))
}

pub(crate) fn manifest_json(
    artifacts: &ArtifactNames,
    source_commit: &str,
    config: Config,
    staged_artifacts: &[StagedArtifact],
) -> String {
    let nt_device_name = format!("\\Device\\{}", artifacts.device_name);
    let dos_device_name = format!("\\DosDevices\\{}", artifacts.device_name);
    let user_device_path = format!("\\\\.\\{}", artifacts.device_name);
    let mut json = format!(
        "{{\n  \"sdk_dll_name\": \"{}\",\n  \"driver_file_name\": \"{}\",\n  \"driver_service_name\": \"{}\",\n  \"device_name\": \"{}\",\n  \"nt_device_name\": \"{}\",\n  \"dos_device_name\": \"{}\",\n  \"user_device_path\": \"{}\",\n  \"source_commit\": \"{}\",\n  \"build_config\": \"{}\",\n  \"artifacts\": [",
        json_string(&artifacts.sdk_dll_name),
        json_string(&artifacts.driver_file_name),
        json_string(&artifacts.driver_service_name),
        json_string(&artifacts.device_name),
        json_string(&nt_device_name),
        json_string(&dos_device_name),
        json_string(&user_device_path),
        json_string(source_commit),
        config.as_str(),
    );

    for (index, artifact) in staged_artifacts.iter().enumerate() {
        if index == 0 {
            json.push('\n');
        } else {
            json.push_str(",\n");
        }
        json.push_str(&format!(
            "    {{\n      \"name\": \"{}\",\n      \"size_bytes\": {},\n      \"sha256\": \"{}\"\n    }}",
            json_string(&artifact.name),
            artifact.size_bytes,
            json_string(&artifact.sha256),
        ));
    }

    json.push_str("\n  ]\n}\n");
    json
}

pub(crate) fn json_string(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_ascii_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::ArtifactNames;
    use crate::cli::ArtifactArgs;

    #[test]
    fn manifest_json_matches_contract() {
        let artifacts = ArtifactNames::from_args(&ArtifactArgs {
            sdk_dll_name: Some("ExampleSdk.dll".to_string()),
            driver_file_name: Some("ExampleDriver.sys".to_string()),
            driver_service_name: Some("ExampleService".to_string()),
            device_name: Some("ExampleDevice".to_string()),
        })
        .unwrap();
        let staged_artifacts = [StagedArtifact {
            name: "ExampleSdk.dll".to_string(),
            size_bytes: 4,
            sha256: "0123456789abcdef".to_string(),
        }];

        assert_eq!(
            manifest_json(&artifacts, "abc123", Config::Release, &staged_artifacts),
            "{\n  \"sdk_dll_name\": \"ExampleSdk.dll\",\n  \"driver_file_name\": \"ExampleDriver.sys\",\n  \"driver_service_name\": \"ExampleService\",\n  \"device_name\": \"ExampleDevice\",\n  \"nt_device_name\": \"\\\\Device\\\\ExampleDevice\",\n  \"dos_device_name\": \"\\\\DosDevices\\\\ExampleDevice\",\n  \"user_device_path\": \"\\\\\\\\.\\\\ExampleDevice\",\n  \"source_commit\": \"abc123\",\n  \"build_config\": \"release\",\n  \"artifacts\": [\n    {\n      \"name\": \"ExampleSdk.dll\",\n      \"size_bytes\": 4,\n      \"sha256\": \"0123456789abcdef\"\n    }\n  ]\n}\n"
        );
    }
}
