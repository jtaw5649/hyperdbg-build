use std::fs;

use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};

use crate::config::{Config, PLATFORM};
use crate::env::BuildEnv;
use crate::fsutil::{require_file, wine_path};
use crate::process::run_logged;

pub(crate) struct AsmItem {
    project: &'static str,
    source: &'static str,
}

pub(crate) const ASM_ITEMS: &[AsmItem] = &[
    AsmItem::new("hyperhv", "hyperdbg/hyperhv/code/assembly/AsmCommon.asm"),
    AsmItem::new("hyperhv", "hyperdbg/hyperhv/code/assembly/AsmHooks.asm"),
    AsmItem::new("hyperhv", "hyperdbg/hyperhv/code/assembly/AsmEpt.asm"),
    AsmItem::new(
        "hyperhv",
        "hyperdbg/hyperhv/code/assembly/AsmSegmentRegs.asm",
    ),
    AsmItem::new(
        "hyperhv",
        "hyperdbg/hyperhv/code/assembly/AsmVmexitHandler.asm",
    ),
    AsmItem::new(
        "hyperhv",
        "hyperdbg/hyperhv/code/assembly/AsmVmxContextState.asm",
    ),
    AsmItem::new(
        "hyperhv",
        "hyperdbg/hyperhv/code/assembly/AsmVmxOperation.asm",
    ),
    AsmItem::new(
        "hyperhv",
        "hyperdbg/hyperhv/code/assembly/AsmInterruptHandlers.asm",
    ),
    AsmItem::new("hyperkd", "hyperdbg/hyperkd/code/assembly/AsmDebugger.asm"),
    AsmItem::new(
        "libhyperdbg",
        "hyperdbg/libhyperdbg/code/assembly/asm-vmx-checks.asm",
    ),
    AsmItem::new(
        "hyperdbg-test",
        "hyperdbg/hyperdbg-test/code/assembly/asm-test.asm",
    ),
];

impl AsmItem {
    const fn new(project: &'static str, source: &'static str) -> Self {
        Self { project, source }
    }

    fn stem(&self) -> Result<&str> {
        Utf8Path::new(self.source)
            .file_stem()
            .ok_or_else(|| anyhow!("asm source has no file stem: {}", self.source))
    }
}

pub(crate) fn preassemble_asm(
    env: &BuildEnv,
    log_dir: &Utf8Path,
    config: Config,
    target_filter: Option<&str>,
) -> Result<()> {
    let mut selected = Vec::new();
    for item in ASM_ITEMS {
        if target_filter.is_none_or(|target| target == item.project) {
            selected.push(item);
        }
    }
    if selected.is_empty() {
        return Ok(());
    }

    for item in selected {
        let source = env.repo_root.join(item.source);
        require_file(&source, "asm source")?;
        let object = asm_object_path(env, item, config)?;
        let parent = object
            .parent()
            .ok_or_else(|| anyhow!("object path has no parent: {object}"))?;
        fs::create_dir_all(parent.as_std_path())
            .with_context(|| format!("failed to create object dir {parent}"))?;

        let mut command = env.command(&env.ml64)?;
        command
            .arg("/nologo")
            .arg("/c")
            .arg("/Zi")
            .arg(format!("/Fo{}", wine_path(&object)?))
            .arg(format!("/Ta{}", wine_path(&source)?));

        let log = log_dir.join(format!("ml64-{}-{}.log", item.project, item.stem()?));
        println!("assembling {} -> {}", item.source, object);
        run_logged(command, &log)
            .with_context(|| format!("ml64 failed for {}; see {log}", item.source))?;
    }

    Ok(())
}

pub(crate) fn asm_object_path(
    env: &BuildEnv,
    item: &AsmItem,
    config: Config,
) -> Result<Utf8PathBuf> {
    Ok(env
        .repo_root
        .join("hyperdbg/build/obj")
        .join(item.project)
        .join(PLATFORM)
        .join(config.as_str())
        .join(format!("{}.obj", item.stem()?)))
}
