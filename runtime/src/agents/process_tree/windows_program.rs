//! Windows program resolution for `ProcessTree::spawn` callers.
//!
//! Rust's `Command` searches PATH for `<name>.exe` only, while the PowerShell
//! gate this replaced resolved bare names over PATH × PATHEXT — that is how
//! `codex` finds the `codex.cmd` shim npm installs next to it. This module
//! keeps that lookup working without a shell in between.
//!
//! A resolved `.cmd`/`.bat` is run through an explicit `cmd.exe /d /s /c`
//! line built here rather than relying on std's implicit batch wrapping:
//! std quotes args for `cmd /c` but does not escape `&`, `|`, `<` or `>`
//! against the second parse the batch file's invocation line goes through,
//! so a metacharacter argument could split into another command.
//! [`escape_batch_token`] caret-escapes at both parse levels.

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
    /// A bare extensionless name resolved to an image through
    /// PATH × PATHEXT — rebuild the command around it.
    Resolved(PathBuf),
    /// A bare extensionless name resolved to a batch file — run it through
    /// `cmd.exe /d /s /c` with the line [`batch_command_line`] builds.
    Batch(PathBuf),
}

/// Rebuild `command` around its resolved program path when the program is a
/// bare name that needs PATH × PATHEXT (e.g. `codex` → `codex.cmd`).
///
/// Only program, args, env vars and current_dir are carried over: the caller
/// configures stdio on the returned command and passes creation flags to
/// `ProcessTree::spawn`. `raw_arg` arguments lose their rawness on a `.cmd`
/// target (they arrive through `get_args` as plain strings and are quoted
/// again), and `env_clear` is not observable on a built `Command` — neither
/// survives the rebuild.
pub(super) fn resolve_command(command: Command) -> io::Result<Command> {
    let resolution = resolve(command.as_std())?;
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

    let mut resolved = match resolution {
        Resolution::Unchanged => return Ok(command),
        Resolution::Resolved(program) => {
            let mut resolved = Command::new(program);
            resolved.args(args);
            resolved
        }
        Resolution::Batch(program) => {
            let line = batch_command_line(program.as_os_str(), &args)?;
            let mut resolved = Command::new(command_prompt()?);
            resolved.args(["/d", "/s", "/e:ON", "/v:OFF", "/c"]);
            // The /c line is one verbatim tail: cmd.exe parses it itself, so
            // it must not go through argv quoting again.
            use std::os::windows::process::CommandExt;
            resolved.as_std_mut().raw_arg(line);
            resolved
        }
    };
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

/// `%SystemDirectory%\cmd.exe` — the same interpreter std picks for batch
/// files: fixed by the OS, not inherited through PATH or COMSPEC.
fn command_prompt() -> io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::System::SystemInformation::GetSystemDirectoryW;

    let mut buffer = vec![0u16; 260];
    let len = unsafe { GetSystemDirectoryW(Some(&mut buffer)) } as usize;
    if len == 0 {
        return Err(io::Error::last_os_error());
    }
    if len > buffer.len() {
        // The buffer was too small; the return value is the required length
        // including the terminator.
        buffer.resize(len, 0);
        let len = unsafe { GetSystemDirectoryW(Some(&mut buffer)) } as usize;
        if len == 0 || len > buffer.len() {
            return Err(io::Error::last_os_error());
        }
        buffer.truncate(len);
    } else {
        buffer.truncate(len);
    }
    Ok(PathBuf::from(OsString::from_wide(&buffer)).join("cmd.exe"))
}

/// The `"<line>"` tail for `cmd.exe /d /s /c`: `/s` makes cmd strip exactly
/// the first and last quote, so the line is wrapped in one outer pair and
/// every token inside carries its own quoting.
fn batch_command_line(program: &OsStr, args: &[OsString]) -> io::Result<OsString> {
    let mut line = OsString::from("\"");
    line.push(escape_batch_token(program)?);
    for arg in args {
        line.push(" ");
        line.push(escape_batch_token(arg)?);
    }
    line.push("\"");
    Ok(line)
}

/// One quoted token inside a `cmd.exe /s /c` line.
///
/// The line is parsed twice: once by cmd.exe when it reads the `/c` string,
/// and again when it builds the batch file's invocation line. Inside quotes
/// cmd still treats a bare `&`, `|`, `<` or `>` as a command separator at the
/// second pass, so every metacharacter is written `^X` — the caret survives
/// the first parse literally and is consumed as an escape in the second.
/// `"` is written `""`: the doubled quote is cmd's own quote escape, which is
/// why a batch shim that forwards `%1`/`%*` hands the original argument to a
/// real program (an .exe sees `""` inside quotes as a literal `"`). `\r`,
/// `\n` and NUL cannot be represented on a command line and fail closed.
pub(super) fn escape_batch_token(arg: &OsStr) -> io::Result<OsString> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    const CARET: u16 = '^' as u16;
    const QUOTE: u16 = '"' as u16;
    let metachar = |unit: u16| {
        // `!` is escaped for callers whose cmd enables delayed expansion;
        // `^` itself must double so it survives to the batch line as one.
        b"^&|<>()%!".iter().any(|&byte| unit == byte as u16)
    };

    let mut out = vec![QUOTE];
    for unit in arg.encode_wide() {
        if unit == QUOTE {
            out.push(QUOTE);
        } else if unit == 0 || unit == '\r' as u16 || unit == '\n' as u16 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "argument cannot be passed on a batch command line: `{}`",
                    arg.to_string_lossy()
                ),
            ));
        } else if metachar(unit) {
            out.push(CARET);
        }
        out.push(unit);
    }
    out.push(QUOTE);
    Ok(OsString::from_wide(&out))
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

/// The PATHEXT entry that won the lookup decides how the file spawns.
fn classify(candidate: PathBuf) -> io::Result<Resolution> {
    match extension(&candidate).as_deref() {
        Some("exe" | "com") => Ok(Resolution::Resolved(candidate)),
        Some("cmd" | "bat") => Ok(Resolution::Batch(candidate)),
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
