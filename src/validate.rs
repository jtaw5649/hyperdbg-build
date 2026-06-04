use anyhow::{Result, bail};
use camino::Utf8Path;

pub(crate) fn validate_filename(
    value: &str,
    extension: &str,
    forbidden: &str,
    label: &str,
) -> Result<String> {
    validate_manifest_filename(label, value)?;
    if !value.to_ascii_lowercase().ends_with(extension) {
        bail!("{label} must end with {extension}: {value:?}");
    }
    let token = Utf8Path::new(forbidden)
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .unwrap_or(forbidden);
    if value
        .to_ascii_lowercase()
        .contains(&token.to_ascii_lowercase())
    {
        bail!("{label} must not contain default token {token:?}: {value:?}");
    }
    Ok(value.to_string())
}

pub(crate) fn validate_service_name(value: &str) -> Result<String> {
    validate_manifest_label("driver service name", value)?;
    if value.to_ascii_lowercase().contains("hyperkd") {
        bail!(
            "driver service name must not contain default token {:?}: {value:?}",
            "hyperkd"
        );
    }
    Ok(value.to_string())
}

pub(crate) fn validate_device_name(value: &str) -> Result<String> {
    validate_manifest_label("device name", value)?;
    if value
        .to_ascii_lowercase()
        .contains(&"HyperDbgDebuggerDevice".to_ascii_lowercase())
    {
        bail!(
            "device name must not contain default token {:?}: {value:?}",
            "HyperDbgDebuggerDevice"
        );
    }
    Ok(value.to_string())
}

pub(crate) fn validate_manifest_filename(label: &str, value: &str) -> Result<()> {
    validate_ascii_basename(value, label)?;
    if value.chars().any(|ch| {
        ch.is_ascii_control() || matches!(ch, ':' | '*' | '?' | '"' | '<' | '>' | '|' | ';')
    }) {
        bail!("{label} must be a safe staged file name: {value:?}");
    }
    Ok(())
}

pub(crate) fn validate_manifest_extension(label: &str, value: &str, extension: &str) -> Result<()> {
    if !value.to_ascii_lowercase().ends_with(extension) {
        bail!("manifest {label} must end with {extension}: {value:?}");
    }
    Ok(())
}

pub(crate) fn validate_manifest_label(label: &str, value: &str) -> Result<()> {
    validate_ascii_basename(value, label)?;
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        bail!("{label} may only contain ASCII letters, digits, '_', '-', and '.': {value:?}");
    }
    Ok(())
}

fn validate_ascii_basename(value: &str, label: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{label} must not be empty");
    }
    if !value.is_ascii() {
        bail!("{label} must be ASCII: {value:?}");
    }
    if value.contains('/') || value.contains('\\') {
        bail!("{label} must be a basename without path separators: {value:?}");
    }
    if value.chars().any(|ch| ch.is_ascii_control()) {
        bail!("{label} must not contain ASCII control characters: {value:?}");
    }
    if value == "." || value == ".." {
        bail!("{label} must be a basename: {value:?}");
    }
    Ok(())
}
