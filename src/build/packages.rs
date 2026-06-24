use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};

use crate::env::BuildEnv;
use crate::process::run_logged;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct NugetPackage {
    id: String,
    version: String,
}

pub(crate) fn restore_packages(env: &BuildEnv, log_dir: &Utf8Path) -> Result<()> {
    let packages = read_packages(env)?;
    if packages.is_empty() {
        return Ok(());
    }

    let packages_dir = env.repo_root.join("hyperdbg/packages");
    let cache_dir = env.out_dir.join("nuget");
    fs::create_dir_all(packages_dir.as_std_path())
        .with_context(|| format!("failed to create {packages_dir}"))?;
    fs::create_dir_all(cache_dir.as_std_path())
        .with_context(|| format!("failed to create {cache_dir}"))?;

    for package in packages {
        restore_package(&package, &packages_dir, &cache_dir, log_dir)?;
    }

    Ok(())
}

fn read_packages(env: &BuildEnv) -> Result<Vec<NugetPackage>> {
    let mut configs = Vec::new();
    collect_package_configs(&env.repo_root.join("hyperdbg"), &mut configs)?;

    let mut packages = BTreeSet::new();
    for config in configs {
        let content = fs::read_to_string(config.as_std_path())
            .with_context(|| format!("failed to read {config}"))?;
        for package in
            parse_packages_config(&content).with_context(|| format!("failed to parse {config}"))?
        {
            packages.insert(package);
        }
    }

    Ok(packages.into_iter().collect())
}

fn collect_package_configs(dir: &Utf8Path, configs: &mut Vec<Utf8PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir.as_std_path()).with_context(|| format!("failed to read {dir}"))? {
        let entry = entry.with_context(|| format!("failed to read entry in {dir}"))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read file type for {:?}", entry.path()))?;
        let path = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|path| anyhow!("non-UTF-8 package path: {:?}", path))?;

        if file_type.is_dir() {
            if !matches!(path.file_name(), Some("build" | "packages")) {
                collect_package_configs(&path, configs)?;
            }
        } else if file_type.is_file() && path.file_name() == Some("packages.config") {
            configs.push(path);
        }
    }
    Ok(())
}

fn parse_packages_config(content: &str) -> Result<Vec<NugetPackage>> {
    let mut packages = Vec::new();
    for line in content.lines().map(str::trim) {
        if !line.starts_with("<package ") {
            continue;
        }

        let id =
            xml_attr(line, "id").with_context(|| format!("package line missing id: {line}"))?;
        let version = xml_attr(line, "version")
            .with_context(|| format!("package line missing version: {line}"))?;
        packages.push(NugetPackage { id, version });
    }
    Ok(packages)
}

fn xml_attr(line: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn restore_package(
    package: &NugetPackage,
    packages_dir: &Utf8Path,
    cache_dir: &Utf8Path,
    log_dir: &Utf8Path,
) -> Result<()> {
    let package_dir = packages_dir.join(package_dir_name(package));
    if package_dir.is_dir() && package_dir_has_entries(&package_dir)? {
        println!("package present: {package_dir}");
        return Ok(());
    }

    let nupkg = cache_dir.join(nupkg_name(package));
    if !nupkg.is_file() {
        download_package(package, &nupkg, log_dir)?;
    }

    let temp_dir = packages_dir.join(format!(".restore-{}", safe_name(package)));
    if temp_dir.exists() {
        fs::remove_dir_all(temp_dir.as_std_path())
            .with_context(|| format!("failed to remove old temp dir {temp_dir}"))?;
    }
    if package_dir.exists() {
        fs::remove_dir_all(package_dir.as_std_path())
            .with_context(|| format!("failed to remove incomplete package dir {package_dir}"))?;
    }
    fs::create_dir_all(temp_dir.as_std_path())
        .with_context(|| format!("failed to create {temp_dir}"))?;

    extract_package(package, &nupkg, &temp_dir, log_dir)?;
    fs::rename(temp_dir.as_std_path(), package_dir.as_std_path())
        .with_context(|| format!("failed to move restored package into {package_dir}"))?;
    println!("package restored: {package_dir}");
    Ok(())
}

fn package_dir_has_entries(package_dir: &Utf8Path) -> Result<bool> {
    Ok(fs::read_dir(package_dir.as_std_path())
        .with_context(|| format!("failed to read {package_dir}"))?
        .next()
        .transpose()
        .with_context(|| format!("failed to read entry in {package_dir}"))?
        .is_some())
}

fn download_package(package: &NugetPackage, nupkg: &Utf8Path, log_dir: &Utf8Path) -> Result<()> {
    let parent = nupkg
        .parent()
        .ok_or_else(|| anyhow!("nupkg path has no parent: {nupkg}"))?;
    fs::create_dir_all(parent.as_std_path())
        .with_context(|| format!("failed to create {parent}"))?;

    let temp = nupkg.with_extension("nupkg.tmp");
    if temp.exists() {
        fs::remove_file(temp.as_std_path())
            .with_context(|| format!("failed to remove old temp file {temp}"))?;
    }

    let mut command = Command::new("curl");
    command
        .arg("--fail")
        .arg("--location")
        .arg("--retry")
        .arg("3")
        .arg("--output")
        .arg(temp.as_str())
        .arg(package_url(package));
    run_logged(
        command,
        &log_dir.join(format!("nuget-download-{}.log", safe_name(package))),
    )
    .with_context(|| format!("failed to download {} {}", package.id, package.version))?;

    fs::rename(temp.as_std_path(), nupkg.as_std_path())
        .with_context(|| format!("failed to cache restored package at {nupkg}"))?;
    Ok(())
}

fn extract_package(
    package: &NugetPackage,
    nupkg: &Utf8Path,
    dest: &Utf8Path,
    log_dir: &Utf8Path,
) -> Result<()> {
    let mut command = Command::new("unzip");
    command
        .arg("-q")
        .arg("-o")
        .arg(nupkg.as_str())
        .arg("-d")
        .arg(dest.as_str());
    run_logged(
        command,
        &log_dir.join(format!("nuget-extract-{}.log", safe_name(package))),
    )
    .with_context(|| format!("failed to extract {} {}", package.id, package.version))
}

fn package_url(package: &NugetPackage) -> String {
    let id = package.id.to_ascii_lowercase();
    let version = package.version.to_ascii_lowercase();
    format!("https://api.nuget.org/v3-flatcontainer/{id}/{version}/{id}.{version}.nupkg")
}

fn nupkg_name(package: &NugetPackage) -> String {
    format!("{}.{}.nupkg", package.id, package.version)
}

fn package_dir_name(package: &NugetPackage) -> String {
    format!("{}.{}", package.id, package.version)
}

fn safe_name(package: &NugetPackage) -> String {
    package_dir_name(package)
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_packages_config_entries() {
        let packages = parse_packages_config(
            r#"<?xml version="1.0" encoding="utf-8"?>
<packages>
  <package id="Microsoft.Windows.SDK.CPP" version="10.0.28000.1839" targetFramework="native" />
  <package id="Microsoft.Windows.WDK.x64" version="10.0.28000.1839" targetFramework="native" />
</packages>
"#,
        )
        .unwrap();

        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].id, "Microsoft.Windows.SDK.CPP");
        assert_eq!(packages[1].version, "10.0.28000.1839");
    }

    #[test]
    fn builds_flat_container_url() {
        let package = NugetPackage {
            id: "Microsoft.Windows.WDK.x64".to_string(),
            version: "10.0.28000.1839".to_string(),
        };

        assert_eq!(
            package_url(&package),
            "https://api.nuget.org/v3-flatcontainer/microsoft.windows.wdk.x64/10.0.28000.1839/microsoft.windows.wdk.x64.10.0.28000.1839.nupkg"
        );
    }
}
