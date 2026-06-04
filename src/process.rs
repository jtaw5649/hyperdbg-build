use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::process::{Command, ExitStatus, Stdio};

use anyhow::{Context, Result, anyhow};
use camino::Utf8Path;

pub(crate) fn run_logged(mut command: Command, log: &Utf8Path) -> Result<()> {
    let parent = log
        .parent()
        .ok_or_else(|| anyhow!("log path has no parent: {log}"))?;
    fs::create_dir_all(parent.as_std_path())
        .with_context(|| format!("failed to create {parent}"))?;

    let mut file =
        fs::File::create(log.as_std_path()).with_context(|| format!("failed to create {log}"))?;
    let command_line = format_command(&command);
    writeln!(file, "command: {command_line}")?;
    writeln!(file, "status: running")?;
    writeln!(file, "\n--- output ---")?;
    file.flush()?;

    let stdout = fs::OpenOptions::new()
        .append(true)
        .open(log.as_std_path())
        .with_context(|| format!("failed to reopen {log} for stdout"))?;
    let stderr = fs::OpenOptions::new()
        .append(true)
        .open(log.as_std_path())
        .with_context(|| format!("failed to reopen {log} for stderr"))?;
    let status = match command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .status()
    {
        Ok(status) => status,
        Err(err) => {
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(log.as_std_path())
                .with_context(|| format!("failed to reopen {log} for spawn error"))?;
            writeln!(file, "\n--- status ---")?;
            writeln!(file, "spawn failed: {err}")?;
            return Err(err).with_context(|| format!("failed to run {command_line}"));
        }
    };

    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(log.as_std_path())
        .with_context(|| format!("failed to reopen {log} for status"))?;
    writeln!(file, "\n--- status ---")?;
    writeln!(file, "{}", status_text(status))?;

    if status.success() {
        println!("ok: {log}");
        Ok(())
    } else {
        Err(anyhow!("command exited with {}", status_text(status)))
    }
}

pub(crate) fn status_text(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exit {code}"),
        None => "terminated by signal".to_string(),
    }
}

fn format_command(command: &Command) -> String {
    let mut parts = Vec::new();
    parts.push(format_os_arg(command.get_program()));
    for arg in command.get_args() {
        parts.push(format_os_arg(arg));
    }
    parts.join(" ")
}

fn format_os_arg(arg: &OsStr) -> String {
    let value = arg.to_string_lossy();
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "@%_+=:,./-\\".contains(ch))
    {
        return value.into_owned();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
