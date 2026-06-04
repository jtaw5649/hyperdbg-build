use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use camino::{Utf8Path, Utf8PathBuf};
use sha2::{Digest, Sha256};

pub(crate) fn sha256_file(path: &Utf8Path) -> Result<String> {
    let mut file = fs::File::open(path.as_std_path())
        .with_context(|| format!("failed to open staged artifact {path}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read staged artifact {path}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn copy_dir_recursive(source: &Utf8Path, dest: &Utf8Path) -> Result<()> {
    fs::create_dir_all(dest.as_std_path()).with_context(|| format!("failed to create {dest}"))?;
    for entry in
        fs::read_dir(source.as_std_path()).with_context(|| format!("failed to read {source}"))?
    {
        let entry = entry.with_context(|| format!("failed to read entry in {source}"))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read file type for {:?}", entry.path()))?;
        let source_path = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|path| anyhow!("non-UTF-8 path in overlay source: {:?}", path))?;
        let dest_path = dest.join(entry.file_name().to_string_lossy().as_ref());
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &dest_path)?;
        } else if file_type.is_file() {
            fs::copy(source_path.as_std_path(), dest_path.as_std_path())
                .with_context(|| format!("failed to copy {source_path} to {dest_path}"))?;
        }
    }
    Ok(())
}

pub(crate) fn copy_files_with_extension(
    source: &Utf8Path,
    dest: &Utf8Path,
    extension: &str,
) -> Result<()> {
    fs::create_dir_all(dest.as_std_path()).with_context(|| format!("failed to create {dest}"))?;
    for entry in
        fs::read_dir(source.as_std_path()).with_context(|| format!("failed to read {source}"))?
    {
        let entry = entry.with_context(|| format!("failed to read entry in {source}"))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read file type for {:?}", entry.path()))?;
        let source_path = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|path| anyhow!("non-UTF-8 script path: {:?}", path))?;
        let dest_path = dest.join(entry.file_name().to_string_lossy().as_ref());
        if file_type.is_dir() {
            copy_files_with_extension(&source_path, &dest_path, extension)?;
        } else if file_type.is_file() && source_path.extension() == Some(extension) {
            fs::copy(source_path.as_std_path(), dest_path.as_std_path())
                .with_context(|| format!("failed to copy {source_path} to {dest_path}"))?;
        }
    }
    Ok(())
}

pub(crate) fn require_file(path: &Utf8Path, label: &str) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        bail!("{label} not found at {path}")
    }
}

pub(crate) fn require_dir(path: &Utf8Path, label: &str) -> Result<()> {
    if path.is_dir() {
        Ok(())
    } else {
        bail!("{label} not found at {path}")
    }
}

pub(crate) fn run_id() -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX_EPOCH")?;
    Ok(now.as_secs().to_string())
}

pub(crate) fn wine_path(path: &Utf8Path) -> Result<String> {
    let absolute = absolute_path(path)?;
    let mut converted = String::from("Z:");
    for part in absolute.as_str().split('/') {
        if !part.is_empty() {
            converted.push('\\');
            converted.push_str(part);
        }
    }
    Ok(converted)
}

pub(crate) fn wine_dir_path(path: &Utf8Path) -> Result<String> {
    let mut converted = wine_path(path)?;
    if !converted.ends_with('\\') {
        converted.push('\\');
    }
    Ok(converted)
}

fn absolute_path(path: &Utf8Path) -> Result<Utf8PathBuf> {
    let std_path: PathBuf = if path.is_absolute() {
        path.as_std_path().to_path_buf()
    } else {
        env::current_dir()
            .context("failed to read current dir")?
            .join(path.as_std_path())
    };
    Utf8PathBuf::from_path_buf(std_path).map_err(|path| anyhow!("non-UTF-8 path: {:?}", path))
}
