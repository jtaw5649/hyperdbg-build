use std::env;
use std::ffi::OsString;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use camino::{Utf8Path, Utf8PathBuf};

use crate::config::{Config, PLATFORM};
use crate::fsutil::{require_dir, require_file};
use crate::process::status_text;

pub(crate) struct BuildEnv {
    pub(crate) repo_root: Utf8PathBuf,
    pub(crate) helper_root: Utf8PathBuf,
    pub(crate) solution: Utf8PathBuf,
    pub(crate) out_dir: Utf8PathBuf,
    pub(crate) msvc_root: Utf8PathBuf,
    pub(crate) wrapper_dir: Utf8PathBuf,
    pub(crate) msbuild: Utf8PathBuf,
    pub(crate) ml64: Utf8PathBuf,
    pub(crate) wineprefix: Option<Utf8PathBuf>,
}

impl BuildEnv {
    pub(crate) fn detect() -> Result<Self> {
        let helper_root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = env_path("HYPERDBG_BUILD_REPO_ROOT")?
            .ok_or_else(|| anyhow!("HyperDbg repo root not found; set HYPERDBG_BUILD_REPO_ROOT"))?;
        let solution = repo_root.join("hyperdbg/hyperdbg.sln");
        let out_dir = helper_root.join("out");

        let msvc_root = env_path("HYPERDBG_BUILD_MSVC_ROOT")?
            .ok_or_else(|| anyhow!("MSVC root not found; set HYPERDBG_BUILD_MSVC_ROOT"))?;
        let wrapper_dir = msvc_root.join("bin/x64");
        let msbuild = wrapper_dir.join("msbuild");
        let ml64 = wrapper_dir.join("ml64");
        let wineprefix = env_path("HYPERDBG_BUILD_WINEPREFIX")?;

        Ok(Self {
            repo_root,
            helper_root,
            solution,
            out_dir,
            msvc_root,
            wrapper_dir,
            msbuild,
            ml64,
            wineprefix,
        })
    }

    pub(crate) fn require_tools(&self) -> Result<()> {
        require_file(&self.solution, "solution")?;
        require_dir(&self.wrapper_dir, "MSVC wrapper dir")?;
        require_file(&self.msbuild, "msbuild wrapper")?;
        require_file(&self.ml64, "ml64 wrapper")?;
        Ok(())
    }

    pub(crate) fn command(&self, program: &Utf8Path) -> Result<Command> {
        let mut command = Command::new(program.as_std_path());
        command.current_dir(self.repo_root.as_std_path());
        command.env("PATH", self.child_path()?);
        if let Some(wineprefix) = &self.wineprefix {
            command.env("WINEPREFIX", wineprefix.as_str());
        }
        Ok(command)
    }

    pub(crate) fn log_dir(&self, run_id: &str) -> Utf8PathBuf {
        self.out_dir.join("logs").join(run_id)
    }

    fn child_path(&self) -> Result<OsString> {
        let mut paths = vec![self.wrapper_dir.as_std_path().to_path_buf()];
        if let Some(existing) = env::var_os("PATH") {
            paths.extend(env::split_paths(&existing));
        }
        env::join_paths(paths).context("failed to build child PATH")
    }
}

pub(crate) struct StageEnv {
    pub(crate) helper_root: Utf8PathBuf,
    pub(crate) out_dir: Utf8PathBuf,
}

impl StageEnv {
    pub(crate) fn detect() -> Result<Self> {
        let helper_root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let out_dir = helper_root.join("out");
        Ok(Self {
            helper_root,
            out_dir,
        })
    }

    pub(crate) fn stage_dir(&self, config: Config) -> Utf8PathBuf {
        self.out_dir.join("stage").join(config.as_str())
    }
}

pub(crate) fn print_env(env: &BuildEnv) -> Result<()> {
    println!("repo root: {}", env.repo_root);
    println!("solution: {}", env.solution);
    println!("MSVC root: {}", env.msvc_root);
    println!("wrapper dir: {}", env.wrapper_dir);
    println!("msbuild: {}", env.msbuild);
    println!("ml64: {}", env.ml64);
    println!(
        "wineprefix: {}",
        env.wineprefix
            .as_deref()
            .unwrap_or(Utf8Path::new("<unset>"))
    );

    env.require_tools()?;
    let output = env
        .command(&env.msbuild)?
        .arg("/version")
        .output()
        .context("failed to run msbuild /version")?;
    println!("msbuild /version status: {}", status_text(output.status));
    println!(
        "msbuild /version stdout:\n{}",
        String::from_utf8_lossy(&output.stdout).trim()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        if !stderr.trim().is_empty() {
            println!(
                "msbuild /version warnings: non-empty Wine stderr while probe succeeded:\n{}",
                stderr.trim()
            );
        }
    } else {
        if !stderr.trim().is_empty() {
            println!("msbuild /version stderr:\n{}", stderr.trim());
        }
        bail!(
            "msbuild /version failed with {}",
            status_text(output.status)
        );
    }
    println!("config flags: Configuration={{debug|release}}, Platform={PLATFORM}, serial=true");
    Ok(())
}

fn env_path(name: &str) -> Result<Option<Utf8PathBuf>> {
    match env::var(name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => Ok(Some(Utf8PathBuf::from(value))),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(anyhow!("failed to read {name}: {err}")),
    }
}
