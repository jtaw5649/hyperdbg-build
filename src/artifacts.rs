use anyhow::{Result, bail};

use crate::cli::ArtifactArgs;
use crate::config::Config;
use crate::validate::{validate_device_name, validate_filename, validate_service_name};

pub(crate) const DEFAULT_SDK_DLL_NAME: &str = "libhyperdbg.dll";
pub(crate) const DEFAULT_DRIVER_FILE_NAME: &str = "hyperkd.sys";
pub(crate) const DEFAULT_DRIVER_SERVICE_NAME: &str = "hyperkd";
pub(crate) const DEFAULT_DEVICE_NAME: &str = "HyperDbgDebuggerDevice";

pub(crate) const FIXED_STAGED_ARTIFACTS: &[&str] = &[
    "hyperdbg-cli.exe",
    "hyperdbg-test.exe",
    "script-engine.dll",
    "symbol-parser.dll",
    "pdbex.dll",
    "msdia140.dll",
    "symsrv.dll",
    "hyperhv.dll",
    "hyperlog.dll",
    "hypertrace.dll",
    "hyperevade.dll",
    "kdserial.dll",
];

pub(crate) const RELEASE_FIXED_STAGED_ARTIFACTS: &[&str] = &[
    "hyperdbg-cli.exe",
    "script-engine.dll",
    "symbol-parser.dll",
    "pdbex.dll",
    "msdia140.dll",
    "symsrv.dll",
    "hyperhv.dll",
    "hyperlog.dll",
    "hypertrace.dll",
    "hyperevade.dll",
    "kdserial.dll",
];

pub(crate) struct ArtifactNames {
    pub(crate) sdk_dll_name: String,
    pub(crate) driver_file_name: String,
    pub(crate) driver_service_name: String,
    pub(crate) device_name: String,
}

impl ArtifactNames {
    pub(crate) fn from_args(args: &ArtifactArgs) -> Result<Self> {
        let explicit_count = [
            args.sdk_dll_name.is_some(),
            args.driver_file_name.is_some(),
            args.driver_service_name.is_some(),
            args.device_name.is_some(),
        ]
        .into_iter()
        .filter(|explicit| *explicit)
        .count();

        let (sdk_dll_name, driver_file_name, driver_service_name, device_name) =
            match explicit_count {
                0 => (
                    DEFAULT_SDK_DLL_NAME.to_string(),
                    DEFAULT_DRIVER_FILE_NAME.to_string(),
                    DEFAULT_DRIVER_SERVICE_NAME.to_string(),
                    DEFAULT_DEVICE_NAME.to_string(),
                ),
                4 => (
                    validate_filename(
                        args.sdk_dll_name.as_deref().unwrap_or_default(),
                        ".dll",
                        DEFAULT_SDK_DLL_NAME,
                        "SDK DLL name",
                    )?,
                    validate_filename(
                        args.driver_file_name.as_deref().unwrap_or_default(),
                        ".sys",
                        DEFAULT_DRIVER_FILE_NAME,
                        "driver file name",
                    )?,
                    validate_service_name(args.driver_service_name.as_deref().unwrap_or_default())?,
                    validate_device_name(args.device_name.as_deref().unwrap_or_default())?,
                ),
                _ => bail!(
                    "--sdk-dll-name, --driver-file-name, --driver-service-name, and --device-name must be supplied together"
                ),
            };

        Ok(Self {
            sdk_dll_name,
            driver_file_name,
            driver_service_name,
            device_name,
        })
    }
}

pub(crate) fn fixed_staged_artifacts(config: Config) -> &'static [&'static str] {
    match config {
        Config::Debug => FIXED_STAGED_ARTIFACTS,
        Config::Release => RELEASE_FIXED_STAGED_ARTIFACTS,
    }
}

pub(crate) fn staged_artifact_names(config: Config, artifacts: &ArtifactNames) -> Vec<&str> {
    let mut names = fixed_staged_artifacts(config).to_vec();
    names.push(artifacts.sdk_dll_name.as_str());
    names.push(artifacts.driver_file_name.as_str());
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact_args(
        sdk: Option<&str>,
        driver: Option<&str>,
        service: Option<&str>,
        device: Option<&str>,
    ) -> ArtifactArgs {
        ArtifactArgs {
            sdk_dll_name: sdk.map(str::to_string),
            driver_file_name: driver.map(str::to_string),
            driver_service_name: service.map(str::to_string),
            device_name: device.map(str::to_string),
        }
    }

    #[test]
    fn artifact_args_accept_custom_names() {
        let artifacts = ArtifactNames::from_args(&artifact_args(
            Some("ExampleSdk.dll"),
            Some("ExampleDriver.sys"),
            Some("ExampleService"),
            Some("ExampleDevice"),
        ))
        .unwrap();

        assert_eq!(artifacts.sdk_dll_name, "ExampleSdk.dll");
        assert_eq!(artifacts.driver_file_name, "ExampleDriver.sys");
        assert_eq!(artifacts.driver_service_name, "ExampleService");
        assert_eq!(artifacts.device_name, "ExampleDevice");
    }

    #[test]
    fn artifact_args_keep_default_upstream_names() {
        let artifacts = ArtifactNames::from_args(&ArtifactArgs::default()).unwrap();

        assert_eq!(artifacts.sdk_dll_name, DEFAULT_SDK_DLL_NAME);
        assert_eq!(artifacts.driver_file_name, DEFAULT_DRIVER_FILE_NAME);
        assert_eq!(artifacts.driver_service_name, DEFAULT_DRIVER_SERVICE_NAME);
        assert_eq!(artifacts.device_name, DEFAULT_DEVICE_NAME);
    }

    #[test]
    fn explicit_artifact_args_reject_upstream_names() {
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some(DEFAULT_SDK_DLL_NAME),
                Some("ExampleDriver.sys"),
                Some("ExampleService"),
                Some("ExampleDevice"),
            ))
            .is_err()
        );
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("ExampleSdk.dll"),
                Some(DEFAULT_DRIVER_FILE_NAME),
                Some("ExampleService"),
                Some("ExampleDevice"),
            ))
            .is_err()
        );
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("ExampleSdk.dll"),
                Some("ExampleDriver.sys"),
                Some(DEFAULT_DRIVER_SERVICE_NAME),
                Some("ExampleDevice"),
            ))
            .is_err()
        );
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("ExampleSdk.dll"),
                Some("ExampleDriver.sys"),
                Some("ExampleService"),
                Some(DEFAULT_DEVICE_NAME),
            ))
            .is_err()
        );
    }

    #[test]
    fn explicit_artifact_args_reject_upstream_tokens() {
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("Example-libhyperdbg.dll"),
                Some("ExampleDriver.sys"),
                Some("ExampleService"),
                Some("ExampleDevice"),
            ))
            .is_err()
        );
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("ExampleSdk.dll"),
                Some("Example-hyperkd.sys"),
                Some("ExampleService"),
                Some("ExampleDevice"),
            ))
            .is_err()
        );
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("ExampleSdk.dll"),
                Some("ExampleDriver.sys"),
                Some("Example-hyperkd"),
                Some("ExampleDevice"),
            ))
            .is_err()
        );
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("ExampleSdk.dll"),
                Some("ExampleDriver.sys"),
                Some("ExampleService"),
                Some("Example-HyperDbgDebuggerDevice"),
            ))
            .is_err()
        );
    }

    #[test]
    fn explicit_artifact_args_require_all_names() {
        let values = [
            Some("ExampleSdk.dll"),
            Some("ExampleDriver.sys"),
            Some("ExampleService"),
            Some("ExampleDevice"),
        ];

        for mask in 1..0b1111 {
            let args = artifact_args(
                (mask & 0b001 != 0).then_some(values[0].unwrap()),
                (mask & 0b010 != 0).then_some(values[1].unwrap()),
                (mask & 0b100 != 0).then_some(values[2].unwrap()),
                (mask & 0b1000 != 0).then_some(values[3].unwrap()),
            );

            assert!(ArtifactNames::from_args(&args).is_err());
        }
    }

    #[test]
    fn artifact_args_reject_paths_non_ascii_and_wrong_extensions() {
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("dir/ExampleSdk.dll"),
                Some("ExampleDriver.sys"),
                Some("ExampleService"),
                Some("ExampleDevice"),
            ))
            .is_err()
        );
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("C:ExampleSdk.dll"),
                Some("ExampleDriver.sys"),
                Some("ExampleService"),
                Some("ExampleDevice"),
            ))
            .is_err()
        );
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("ExampleSdk.exe"),
                Some("ExampleDriver.sys"),
                Some("ExampleService"),
                Some("ExampleDevice"),
            ))
            .is_err()
        );
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("ExampleSdk.dll"),
                Some("ExampleDriver.sys"),
                Some("Example/Service"),
                Some("ExampleDevice"),
            ))
            .is_err()
        );
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("ÉxampleSdk.dll"),
                Some("ExampleDriver.sys"),
                Some("ExampleService"),
                Some("ExampleDevice"),
            ))
            .is_err()
        );
        assert!(
            ArtifactNames::from_args(&artifact_args(
                Some("ExampleSdk.dll"),
                Some("ExampleDriver.sys"),
                Some("ExampleService"),
                Some("Example/Device"),
            ))
            .is_err()
        );
    }

    #[test]
    fn explicit_service_name_rejects_injection_characters() {
        for service in ["Example\"Service", "Example;Service"] {
            assert!(
                ArtifactNames::from_args(&artifact_args(
                    Some("ExampleSdk.dll"),
                    Some("ExampleDriver.sys"),
                    Some(service),
                    Some("ExampleDevice"),
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn explicit_filename_args_reject_injection_characters() {
        for sdk_dll_name in ["ExampleSdk;Injected=true.dll", "Example\"Sdk.dll"] {
            assert!(
                ArtifactNames::from_args(&artifact_args(
                    Some(sdk_dll_name),
                    Some("ExampleDriver.sys"),
                    Some("ExampleService"),
                    Some("ExampleDevice"),
                ))
                .is_err()
            );
        }

        for driver_file_name in ["ExampleDriver;Injected=true.sys", "Example\"Driver.sys"] {
            assert!(
                ArtifactNames::from_args(&artifact_args(
                    Some("ExampleSdk.dll"),
                    Some(driver_file_name),
                    Some("ExampleService"),
                    Some("ExampleDevice"),
                ))
                .is_err()
            );
        }

        for device_name in ["ExampleDevice;Injected=true", "Example\"Device"] {
            assert!(
                ArtifactNames::from_args(&artifact_args(
                    Some("ExampleSdk.dll"),
                    Some("ExampleDriver.sys"),
                    Some("ExampleService"),
                    Some(device_name),
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn explicit_artifact_list_uses_only_custom_sdk_and_driver() {
        let artifacts = ArtifactNames::from_args(&artifact_args(
            Some("ExampleSdk.dll"),
            Some("ExampleDriver.sys"),
            Some("ExampleService"),
            Some("ExampleDevice"),
        ))
        .unwrap();
        let names = staged_artifact_names(Config::Debug, &artifacts);

        assert!(names.contains(&"ExampleSdk.dll"));
        assert!(names.contains(&"ExampleDriver.sys"));
        assert!(!names.contains(&DEFAULT_SDK_DLL_NAME));
        assert!(!names.contains(&DEFAULT_DRIVER_FILE_NAME));
    }

    #[test]
    fn fixed_artifact_list_includes_hyperdbg_test_for_debug_only() {
        assert!(fixed_staged_artifacts(Config::Debug).contains(&"hyperdbg-test.exe"));
        assert!(!fixed_staged_artifacts(Config::Release).contains(&"hyperdbg-test.exe"));
    }

    #[test]
    fn release_artifact_list_excludes_hyperdbg_test() {
        let artifacts = ArtifactNames::from_args(&artifact_args(
            Some("ExampleSdk.dll"),
            Some("ExampleDriver.sys"),
            Some("ExampleService"),
            Some("ExampleDevice"),
        ))
        .unwrap();

        let names = staged_artifact_names(Config::Release, &artifacts);

        assert!(!names.contains(&"hyperdbg-test.exe"));
        assert!(names.contains(&"hyperdbg-cli.exe"));
        assert!(names.contains(&"ExampleSdk.dll"));
        assert!(names.contains(&"ExampleDriver.sys"));
    }
}
