//! Windows program resolution for `ProcessTree::spawn` callers.
//!
//! Rust's `Command` searches PATH for `<name>.exe` only, while the PowerShell
//! gate this replaced resolved bare names over PATH × PATHEXT — that is how
//! `codex` finds the `codex.cmd` shim npm installs next to it. This module
//! keeps that lookup working without a shell in between.
//!
//! A resolved `.cmd`/`.bat` path is handed to `Command` as-is: since
//! Rust 1.77.2 (the BatBadBut fix, CVE-2024-24576) std wraps batch files in
//! `cmd.exe` itself and refuses or escapes arguments it cannot pass safely,
//! including across the second parse a batch file's invocation goes through.
//! Hand-rolling that line was tried and is wrong: `%*`-forwarding shims pass
//! cmd's escaping on to the real program.

use std::env;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Component, Path, PathBuf};

use tokio::process::Command;

/// PATHEXT applied when neither the command's env nor the process env sets
/// one — the same default `SearchPath`/cmd use.
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// The outcome of [`resolve`].
#[derive(Debug)]
pub(super) enum Resolution {
    /// The program names a concrete path or carries an explicit extension —
    /// spawn the command unchanged.
    Unchanged,
    /// A bare extensionless name resolved through PATH × PATHEXT — rebuild
    /// the command around the full path. A `.cmd`/`.bat` path goes in
    /// verbatim so std does its own `cmd.exe` wrapping and escaping.
    Resolved(PathBuf),
}

/// Rebuild `command` around its resolved program path when the program is a
/// bare name that needs PATH × PATHEXT (e.g. `codex` → `codex.cmd`).
///
/// Only program, args, env vars and current_dir are carried over: the caller
/// configures stdio on the returned command and passes creation flags to
/// `ProcessTree::spawn`. `raw_arg` arguments lose their rawness (they arrive
/// through `get_args` as plain strings and are quoted again), and `env_clear`
/// is not observable on a built `Command` — neither survives the rebuild.
pub(super) fn resolve_command(command: Command) -> io::Result<Command> {
    let source = command.as_std();
    let args = source
        .get_args()
        .map(OsStr::to_os_string)
        .collect::<Vec<_>>();
    let envs = source
        .get_envs()
        .map(|(key, value)| (key.to_os_string(), value.map(OsStr::to_os_string)))
        .collect::<Vec<_>>();
    let current_dir = source.get_current_dir().map(Path::to_path_buf);
    let Resolution::Resolved(program) = resolve(source)? else {
        return Ok(command);
    };

    let mut resolved = Command::new(program);
    resolved.args(args);
    for (key, value) in envs {
        match value {
            Some(value) => resolved.env(key, value),
            None => resolved.env_remove(key),
        };
    }
    if let Some(dir) = current_dir {
        resolved.current_dir(dir);
    }
    Ok(resolved)
}

/// Resolves `command`'s program the way its spawned environment will see it.
pub(super) fn resolve(command: &std::process::Command) -> io::Result<Resolution> {
    let program = command.get_program();
    let path = Path::new(program);
    if !is_bare_name(path) {
        // A path was given: nothing to look up, but a PowerShell script still
        // cannot be spawned — say so instead of failing cryptically later.
        if extension(path).as_deref() == Some("ps1") {
            return Err(unsupported_extension(path, "ps1"));
        }
        return Ok(Resolution::Unchanged);
    }
    if path.extension().is_some() {
        // `name.ext`: literal file-name lookup over PATH.
        for dir in path_dirs(command) {
            let candidate = dir.join(program);
            if candidate.is_file() {
                return classify(candidate);
            }
        }
        return Err(not_found(program));
    }
    // Bare extensionless name: PATH directories in order, PATHEXT entries in
    // order within each — the same precedence `& program` had.
    let pathext = env_value(command, "PATHEXT").unwrap_or_else(|| OsString::from(DEFAULT_PATHEXT));
    for dir in path_dirs(command) {
        for ext in pathext.to_string_lossy().split(';') {
            if ext.is_empty() {
                continue;
            }
            let mut candidate = dir.join(program).into_os_string();
            if !ext.starts_with('.') {
                candidate.push(".");
            }
            candidate.push(ext);
            let candidate = PathBuf::from(candidate);
            if candidate.is_file() {
                return classify(candidate);
            }
        }
    }
    Err(not_found(program))
}

/// `foo` is a bare name; `.\foo`, `dir/foo` and absolute paths are not.
fn is_bare_name(path: &Path) -> bool {
    let mut components = path.components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

/// The PATHEXT entry that won the lookup decides whether CreateProcess can
/// run the file: `.cmd`/`.bat` count because std wraps them in `cmd.exe`.
fn classify(candidate: PathBuf) -> io::Result<Resolution> {
    match extension(&candidate).as_deref() {
        Some("exe" | "com" | "cmd" | "bat") => Ok(Resolution::Resolved(candidate)),
        Some(extension) => Err(unsupported_extension(&candidate, extension)),
        None => Err(unsupported_extension(&candidate, "")),
    }
}

/// `.ps1` is a script, not an image CreateProcess can run — npm always ships a
/// `.cmd` next to it, so reporting the dead end beats a PowerShell fallback.
fn unsupported_extension(path: &Path, extension: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "program `{}` resolved to `{}`: `.{extension}` files cannot be spawned without a shell \
             (expected .exe, .com, .cmd or .bat)",
            path.file_stem().unwrap_or_default().to_string_lossy(),
            path.display(),
        ),
    )
}

fn not_found(program: &OsStr) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "program not found on PATH × PATHEXT: `{}`",
            program.to_string_lossy()
        ),
    )
}

/// The PATH directories `command` will run with: its own `env("PATH", …)`
/// when set, else the inherited environment.
fn path_dirs(command: &std::process::Command) -> Vec<PathBuf> {
    env_value(command, "PATH")
        .map(|path| {
            env::split_paths(&path)
                .filter(|entry| !entry.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Effective value of `name` in `command`'s spawned environment: the process
/// value, then each `env`/`env_remove` on the command applied in order.
/// (`env_clear` is not observable on a built `Command`; no caller uses it.)
fn env_value(command: &std::process::Command, name: &str) -> Option<OsString> {
    let mut value = env::var_os(name);
    for (key, item) in command.get_envs() {
        if key.as_encoded_bytes().eq_ignore_ascii_case(name.as_bytes()) {
            value = item.map(OsStr::to_os_string);
        }
    }
    value
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
}
