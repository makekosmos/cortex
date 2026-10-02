//! Broker process spawn: detached stdio, confined to the configured roots.

use super::fs_safety::{is_under_configured_root, reject_path};
use super::*;
use std::path::Path;

pub fn spawn_process(
    config: &BrokerConfig,
    executable: &Path,
    args: &[&str],
    cwd: Option<&Path>,
) -> Result<u32, BrokerError> {
    if args.len() > 64
        || args
            .iter()
            .any(|arg| arg.len() > 4096 || arg.chars().any(char::is_control))
    {
        return Err(BrokerError::Invalid("invalid process arguments".into()));
    }
    reject_path(executable)?;
    let executable = std::fs::canonicalize(executable)?;
    if !is_under_configured_root(config, &executable) {
        return Err(BrokerError::Invalid(
            "executable escapes configured roots".into(),
        ));
    }
    if !executable.is_file() {
        return Err(BrokerError::Invalid("executable is not a file".into()));
    }
    let cwd = cwd
        .map(|path| {
            reject_path(path)?;
            let path = std::fs::canonicalize(path)?;
            is_under_configured_root(config, &path)
                .then_some(path)
                .ok_or_else(|| {
                    BrokerError::Invalid("working directory escapes configured roots".into())
                })
        })
        .transpose()?;
    if cwd.as_ref().is_some_and(|path| !path.is_dir()) {
        return Err(BrokerError::Invalid(
            "working directory is not a directory".into(),
        ));
    }
    let mut command = std::process::Command::new(executable);
    command
        .env_clear()
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    for key in [
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMDATA",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "PATH",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    // ponytail: user-launched apps are detached and may outlive the package worker;
    // add an explicit managed-process mode before using this broker for helpers.
    Ok(command.spawn()?.id())
}
