use super::windows_program::{escape_batch_token, resolve, Resolution};
use super::*;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

fn resolved_path(command: &Command) -> Option<PathBuf> {
    match resolve(command.as_std()).unwrap() {
        Resolution::Resolved(path) | Resolution::Batch(path) => Some(path),
        Resolution::Unchanged => None,
    }
}

/// The resolved candidate keeps the PATHEXT entry's letter case, which the
/// case-insensitive file system treats as identical.
fn assert_resolved_eq(command: &Command, expected: PathBuf) {
    let resolved = resolved_path(command).unwrap();
    assert!(
        resolved
            .as_os_str()
            .eq_ignore_ascii_case(expected.as_os_str()),
        "resolved {resolved:?}, expected {expected:?}"
    );
}

#[test]
fn bare_name_obeys_pathext_order() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.exe"), []).unwrap();
    std::fs::write(dir.path().join("probe.cmd"), []).unwrap();
    let mut command = Command::new("probe");
    command.env("PATH", dir.path()).env("PATHEXT", ".CMD;.EXE");
    assert_resolved_eq(&command, dir.path().join("probe.cmd"));

    let mut command = Command::new("probe");
    command.env("PATH", dir.path()).env("PATHEXT", ".EXE;.CMD");
    assert_resolved_eq(&command, dir.path().join("probe.exe"));
}

#[test]
fn bare_name_obeys_path_order() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    std::fs::write(first.path().join("probe.cmd"), []).unwrap();
    std::fs::write(second.path().join("probe.exe"), []).unwrap();
    let path = format!("{};{}", first.path().display(), second.path().display());
    let mut command = Command::new("probe");
    command.env("PATH", &path);
    assert_resolved_eq(&command, first.path().join("probe.cmd"));
}

#[test]
fn bare_name_with_extension_looks_up_literally() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.cmd"), []).unwrap();
    let mut command = Command::new("probe.cmd");
    command.env("PATH", dir.path());
    assert_resolved_eq(&command, dir.path().join("probe.cmd"));
}

#[test]
fn explicit_path_passes_through() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("probe.exe");
    std::fs::write(&exe, []).unwrap();
    let command = Command::new(&exe);
    assert_eq!(resolved_path(&command), None);
}

#[test]
fn missing_program_is_a_clear_error() {
    let command = Command::new("definitely-not-a-real-program-kos294");
    let error = resolve(command.as_std()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(
        error
            .to_string()
            .contains("definitely-not-a-real-program-kos294"),
        "error names the program: {error}"
    );
}

#[test]
fn command_env_path_overrides_process_path() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.cmd"), []).unwrap();
    // No PATH entry outside `dir` can provide `probe`, so a hit proves the
    // command's env PATH is what was searched.
    let mut command = Command::new("probe");
    command.env("PATH", dir.path());
    assert_resolved_eq(&command, dir.path().join("probe.cmd"));

    // And an empty PATH resolves nothing.
    let mut command = Command::new("probe");
    command.env("PATH", "");
    assert!(resolve(command.as_std()).is_err());
}

#[test]
fn ps1_only_match_is_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.ps1"), []).unwrap();
    let mut command = Command::new("probe");
    command
        .env("PATH", dir.path())
        .env("PATHEXT", ".PS1;.CMD;.EXE");
    let error = resolve(command.as_std()).unwrap_err();
    assert!(
        error.to_string().contains("ps1"),
        "error explains .ps1 is unsupported: {error}"
    );
}

#[test]
fn resolve_command_rebuilds_keeping_args_env_and_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let workdir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.exe"), []).unwrap();
    let mut command = Command::new("probe");
    command
        .args(["one", "two three"])
        .env("PATH", dir.path())
        .env("EXTRA", "kept")
        .env_remove("REMOVED")
        .current_dir(workdir.path());

    let resolved = resolve_command(command).unwrap();
    let resolved_std = resolved.as_std();
    assert!(Path::new(resolved_std.get_program())
        .as_os_str()
        .eq_ignore_ascii_case(dir.path().join("probe.exe").as_os_str()));
    assert_eq!(
        resolved_std.get_args().collect::<Vec<_>>(),
        [OsStr::new("one"), OsStr::new("two three")]
    );
    assert_eq!(resolved_std.get_current_dir().unwrap(), workdir.path());
    let envs = resolved_std.get_envs().collect::<Vec<_>>();
    assert!(envs.contains(&(OsStr::new("EXTRA"), Some(OsStr::new("kept")))));
    assert!(envs.contains(&(OsStr::new("REMOVED"), None)));
}

#[test]
fn npm_style_codex_resolves_to_cmd_shim() {
    // npm installs `codex.cmd` (plus a `codex.ps1` we must not pick) next to
    // `codex` shell scripts in %APPDATA%\npm — the shim is the only runnable.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("codex.cmd"), []).unwrap();
    std::fs::write(dir.path().join("codex.ps1"), []).unwrap();
    let mut command = Command::new("codex");
    command.env("PATH", dir.path());

    let resolved = resolve_command(command).unwrap();
    let resolved_std = resolved.as_std();
    assert!(
        Path::new(resolved_std.get_program())
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("cmd.exe")),
        "codex.cmd spawns through cmd.exe, got {:?}",
        resolved_std.get_program()
    );
    let line = resolved_std.get_args().nth(5).unwrap().to_string_lossy();
    assert!(
        line.to_ascii_lowercase().contains("codex.cmd"),
        "/c line names the resolved shim: {line}"
    );
}

#[test]
fn batch_resolution_reroutes_through_cmd_exe() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("probe.cmd");
    std::fs::write(&script, []).unwrap();
    let mut command = Command::new("probe");
    command
        .args(["one", "a&b"])
        .env("PATH", dir.path())
        .env("EXTRA", "kept");

    let resolved = resolve_command(command).unwrap();
    let resolved_std = resolved.as_std();
    assert!(
        Path::new(resolved_std.get_program())
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("cmd.exe")),
        "batch files spawn through cmd.exe, got {:?}",
        resolved_std.get_program()
    );
    let args = resolved_std.get_args().collect::<Vec<_>>();
    // raw_arg'd text still shows up through get_args: the last entry is the
    // whole /c line, quotes and caret escapes included.
    assert_eq!(
        args[..5],
        ["/d", "/s", "/e:ON", "/v:OFF", "/c"].map(OsStr::new)
    );
    let line = args[5].to_string_lossy();
    assert!(
        line.starts_with('"') && line.ends_with('"'),
        "/s strips the outer quote pair: {line}"
    );
    let script_name = script.display().to_string();
    assert!(
        line.to_ascii_lowercase()
            .contains(&format!("\"{}\"", script_name.to_ascii_lowercase())),
        "script path is quoted in the line: {line}"
    );
    assert!(
        line.contains("\"a^&b\""),
        "metacharacters carry caret escapes: {line}"
    );
    assert!(resolved_std
        .get_envs()
        .any(|(key, value)| key == OsStr::new("EXTRA") && value == Some(OsStr::new("kept"))));
}

#[test]
fn batch_token_escaping() {
    // (input, expected token) — the token goes through cmd /c parsing and then
    // the batch invocation parse; verified against cmd.exe on Windows.
    let cases = [
        ("plain", "\"plain\""),
        ("with space", "\"with space\""),
        ("a&b|c>d", "\"a^&b^|c^>d\""),
        ("x^y!(z)", "\"x^^y^!^(z^)\""),
        ("50%", "\"50^%\""),
        // `"` doubles — the batch-level quote escape a downstream .exe
        // collapses back to a literal `"` when it parses argv.
        ("say \"hi\"", "\"say \"\"hi\"\"\""),
        ("a\"&b", "\"a\"\"^&b\""),
        ("a\" b", "\"a\"\" b\""),
        ("", "\"\""),
        ("trailing\\", "\"trailing\\\""),
    ];
    for (input, expected) in cases {
        assert_eq!(
            escape_batch_token(OsStr::new(input)).unwrap(),
            OsStr::new(expected),
            "input: {input:?}"
        );
    }
}

#[test]
fn batch_token_rejects_line_breaks() {
    // `\r`/`\n` would truncate the /c line mid-argument.
    let error = escape_batch_token(OsStr::new("a\rb")).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}
