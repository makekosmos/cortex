use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation")]
enum Request {
    #[serde(rename = "scan_executables")]
    ScanExecutables { root: String },
    #[serde(rename = "copy_backup_directory")]
    CopyBackupDirectory {
        #[serde(rename = "requestPath")]
        request_path: String,
    },
    #[serde(rename = "restore_backup_directory")]
    RestoreBackupDirectory {
        #[serde(rename = "backupRoot")]
        backup_root: String,
    },
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct ExeEntry {
    path: String,
    file_name: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct BackupCopyFile {
    path: String,
    #[serde(rename = "rootLabel")]
    root_label: String,
    #[serde(rename = "relativePath")]
    relative_path: String,
    size: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct BackupManifestEntry {
    #[serde(rename = "backupPath")]
    backup_path: String,
    #[serde(rename = "originalPath")]
    original_path: String,
    size: u64,
    #[serde(default)]
    #[serde(rename = "mtime")]
    mtime: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
struct BackupManifest {
    version: u32,
    files: Vec<BackupManifestEntry>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct BackupCopyHeader {
    destination: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct BackupProgress<'a> {
    stage: &'a str,
    current: &'a str,
    done: usize,
    total: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct BackupProgressEvent<'a> {
    event: &'a str,
    progress: BackupProgress<'a>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ScanEntryEvent<'a> {
    event: &'a str,
    entry: &'a ExeEntry,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct SidecarResponse<T: Serialize> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct BackupCopyData {
    #[serde(rename = "totalBytes")]
    total_bytes: u64,
}

fn success_response<T: Serialize>(data: T) -> SidecarResponse<T> {
    SidecarResponse {
        ok: true,
        data: Some(data),
        error: None,
    }
}

fn error_response(error: impl ToString) -> SidecarResponse<()> {
    SidecarResponse {
        ok: false,
        data: None,
        error: Some(error.to_string()),
    }
}

fn is_hidden_segment(segment: &str) -> bool {
    segment.starts_with('.')
}

fn is_executable_file(file_name: &str) -> bool {
    Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

fn normalize_scan_root(root: &str) -> io::Result<PathBuf> {
    let trimmed = root.trim();
    if trimmed.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Empty scan directory",
        ));
    }

    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn scan_executables<F>(root: &str, mut on_entry: F) -> io::Result<usize>
where
    F: FnMut(ExeEntry) -> io::Result<()>,
{
    let scan_root = normalize_scan_root(root)?;
    let mut stack = vec![scan_root];
    let mut count = 0;

    while let Some(current_dir) = stack.pop() {
        let metadata = match fs::metadata(&current_dir) {
            Ok(metadata) if metadata.is_dir() => metadata,
            _ => continue,
        };

        if !metadata.is_dir() {
            continue;
        }

        let entries = match fs::read_dir(&current_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry_result in entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let file_name = entry.file_name().to_string_lossy().to_string();
            if is_hidden_segment(&file_name) {
                continue;
            }

            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                stack.push(path);
                continue;
            }

            if !file_type.is_file() || !is_executable_file(&file_name) {
                continue;
            }

            count += 1;
            on_entry(ExeEntry {
                path: path.to_string_lossy().to_string(),
                file_name,
            })?;
        }
    }

    Ok(count)
}

fn write_json_line<T: Serialize>(value: &T) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, value)
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    lock.write_all(b"\n")?;
    lock.flush()
}

fn write_scan_entry(entry: &ExeEntry) -> io::Result<()> {
    write_json_line(&ScanEntryEvent {
        event: "scan_entry",
        entry,
    })
}

fn write_success_count(count: usize) -> io::Result<()> {
    write_json_line(&success_response(count))
}

fn write_success_total_bytes(total_bytes: u64) -> io::Result<()> {
    write_json_line(&success_response(BackupCopyData { total_bytes }))
}

fn write_success_bool(value: bool) -> io::Result<()> {
    write_json_line(&success_response(value))
}

fn write_error(error: impl ToString) -> io::Result<()> {
    write_json_line(&error_response(error))
}

fn write_backup_progress(stage: &str, current: &str, done: usize, total: usize) -> io::Result<()> {
    write_json_line(&BackupProgressEvent {
        event: "backup_progress",
        progress: BackupProgress {
            stage,
            current,
            done,
            total,
        },
    })
}

fn parse_request(line: &str) -> Result<Request, String> {
    serde_json::from_str(line).map_err(|error| error.to_string())
}

fn ensure_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

fn build_backup_rel_path(root_label: &str, relative_path: &str) -> String {
    let rel = relative_path
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string();
    if rel.is_empty() {
        return format!("files/{}/file", root_label);
    }
    format!("files/{}/{}", root_label, rel)
}

fn file_mtime_millis(path: &Path) -> Option<u64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    modified
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis()
        .try_into()
        .ok()
}

fn write_backup_readme(destination: &Path) -> io::Result<()> {
    fs::write(
        destination.join("__sqoba_readme.txt"),
        [
            "SQOBA backup format",
            "",
            "This folder contains raw save files plus a manifest.",
            "- __sqoba_manifest.json: list of files and original paths",
            "- files/: backed up files in the same structure as the saves",
            "",
            "To restore manually:",
            "1) Open __sqoba_manifest.json",
            "2) For each entry, copy files/<path> to original_path",
            "",
        ]
        .join("\n"),
    )
}

fn write_backup_manifest(destination: &Path, entries: &[BackupManifestEntry]) -> io::Result<()> {
    let manifest = serde_json::to_string_pretty(&BackupManifest {
        version: 2,
        files: entries.to_vec(),
    })
    .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    fs::write(destination.join("__sqoba_manifest.json"), manifest)
}

fn parse_backup_copy_file(line: &str) -> Result<BackupCopyFile, String> {
    serde_json::from_str(line).map_err(|error| error.to_string())
}

fn read_copy_request(request_path: &str) -> Result<(PathBuf, Vec<BackupCopyFile>), String> {
    let text = fs::read_to_string(request_path).map_err(|error| error.to_string())?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| "missing copy request header".to_string())?;
    let header: BackupCopyHeader =
        serde_json::from_str(header).map_err(|error| error.to_string())?;
    let destination = PathBuf::from(header.destination);
    let mut files = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        files.push(parse_backup_copy_file(trimmed)?);
    }
    Ok((destination, files))
}

fn copy_backup_directory(request_path: &str) -> io::Result<u64> {
    let (destination, files) = read_copy_request(request_path)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    ensure_dir(&destination)?;
    let total = files.len();
    let mut total_bytes = 0;
    let mut entries = Vec::with_capacity(files.len());

    for (index, file) in files.iter().enumerate() {
        let backup_path = build_backup_rel_path(&file.root_label, &file.relative_path);
        let target = destination.join(PathBuf::from(
            backup_path.replace('/', std::path::MAIN_SEPARATOR_STR),
        ));
        if let Some(parent) = target.parent() {
            ensure_dir(parent)?;
        }
        fs::copy(&file.path, &target)?;
        total_bytes += file.size;
        entries.push(BackupManifestEntry {
            backup_path,
            original_path: file.path.clone(),
            size: file.size,
            mtime: file_mtime_millis(Path::new(&file.path)),
        });

        let done = index + 1;
        if done == total || done % 50 == 0 {
            write_backup_progress("copy", &file.path, done, total)?;
        }
    }

    write_backup_manifest(&destination, &entries)?;
    write_backup_readme(&destination)?;
    Ok(total_bytes)
}

fn validate_backup_rel_path(rel: &str) -> io::Result<()> {
    let parts = rel.split('/').filter(|part| !part.is_empty());
    let mut saw_part = false;
    for part in parts {
        saw_part = true;
        if part == "." || part == ".." || part.contains(':') || Path::new(part).is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid backup path in manifest: {rel}"),
            ));
        }
    }
    if !saw_part {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid backup path in manifest: {rel}"),
        ));
    }
    Ok(())
}

fn validate_restore_target_path(original: &str) -> io::Result<PathBuf> {
    if original
        .split(['\\', '/'])
        .any(|part| part == "." || part == "..")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid restore target path in manifest: {original}"),
        ));
    }
    let target = PathBuf::from(original);
    if !target.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid restore target path in manifest: {original}"),
        ));
    }
    Ok(target)
}

fn find_manifest_path(backup_root: &Path) -> Option<PathBuf> {
    for name in ["__sqoba_manifest.json", "__arrancador_manifest.json"] {
        let candidate = backup_root.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn parse_backup_manifest_entries(text: &str) -> Result<Vec<BackupManifestEntry>, String> {
    serde_json::from_str::<BackupManifest>(text)
        .map(|manifest| manifest.files)
        .map_err(|error| error.to_string())
}

fn restore_backup_directory(backup_root: &str) -> io::Result<()> {
    let backup_root = PathBuf::from(backup_root);
    let manifest_path = find_manifest_path(&backup_root)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Backup manifest is missing"))?;
    let manifest_text = fs::read_to_string(manifest_path)?;
    let entries = parse_backup_manifest_entries(&manifest_text)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let total = entries.len();

    for (index, entry) in entries.iter().enumerate() {
        validate_backup_rel_path(&entry.backup_path)?;
        let source = backup_root.join(PathBuf::from(
            entry
                .backup_path
                .replace('/', std::path::MAIN_SEPARATOR_STR),
        ));
        if !source.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Backup file is missing: {}", source.to_string_lossy()),
            ));
        }
        let target = validate_restore_target_path(&entry.original_path)?;
        if let Some(parent) = target.parent() {
            ensure_dir(parent)?;
        }
        fs::copy(&source, &target)?;
        let done = index + 1;
        if done == total || done % 50 == 0 {
            write_backup_progress("restore", &entry.original_path, done, total)?;
        }
    }

    Ok(())
}

fn handle_request(request: Request) -> io::Result<()> {
    match request {
        Request::ScanExecutables { root } => {
            let count = scan_executables(&root, |entry| write_scan_entry(&entry))?;
            write_success_count(count)
        }
        Request::CopyBackupDirectory { request_path } => {
            let total_bytes = copy_backup_directory(&request_path)?;
            write_success_total_bytes(total_bytes)
        }
        Request::RestoreBackupDirectory { backup_root } => {
            restore_backup_directory(&backup_root)?;
            write_success_bool(true)
        }
    }
}

fn serve_requests() {
    let stdin = io::stdin();
    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(line) => line,
            Err(error) => {
                let _ = write_error(error);
                continue;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match parse_request(trimmed) {
            Ok(request) => {
                if let Err(error) = handle_request(request) {
                    let _ = write_error(error);
                }
            }
            Err(error) => {
                let _ = write_error(error);
            }
        }
    }
}

fn run() -> Result<(), String> {
    match std::env::args().nth(1).as_deref() {
        Some("serve") => {
            serve_requests();
            Ok(())
        }
        _ => Err("Missing or unsupported command. Use `serve`.".to_string()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("arrancador-sidecar fatal: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_test_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("arrancador-sidecar-test-{suffix}"));
        fs::create_dir_all(&root).expect("test root should be created");
        root
    }

    #[test]
    fn parse_scan_request_with_escaped_windows_path() {
        let request =
            parse_request(r#"{"operation":"scan_executables","root":"C:\\Games\\Steam"}"#)
                .expect("request should parse");

        assert_eq!(
            request,
            Request::ScanExecutables {
                root: "C:\\Games\\Steam".to_string(),
            },
        );
    }

    #[test]
    fn parse_scan_request_preserves_unicode_json_escapes() {
        let request = parse_request(
            r#"{"operation":"scan_executables","root":"D:\\Games\\\u0418\u0433\u0440\u044b\\\u30c6\u30b9\u30c8"}"#,
        )
        .expect("request should parse");

        assert_eq!(
            request,
            Request::ScanExecutables {
                root: "D:\\Games\\\u{0418}\u{0433}\u{0440}\u{044b}\\\u{30c6}\u{30b9}\u{30c8}"
                    .to_string(),
            },
        );
    }

    #[test]
    fn scan_finds_exe_files_case_insensitively_and_skips_hidden_dirs() {
        let root = make_test_root();
        let nested = root.join("nested");
        let hidden = root.join(".hidden");
        fs::create_dir_all(&nested).expect("nested dir should be created");
        fs::create_dir_all(&hidden).expect("hidden dir should be created");
        fs::write(root.join("game.exe"), b"").expect("exe should be written");
        fs::write(nested.join("tool.EXE"), b"").expect("uppercase exe should be written");
        fs::write(root.join("notes.txt"), b"").expect("txt should be written");
        fs::write(hidden.join("skip.exe"), b"").expect("hidden exe should be written");

        let mut entries = Vec::new();
        let count = scan_executables(root.to_str().expect("utf8 path"), |entry| {
            entries.push(entry);
            Ok(())
        })
        .expect("scan should succeed");

        let _ = fs::remove_dir_all(&root);

        assert_eq!(count, 2);
        let mut names = entries
            .into_iter()
            .map(|entry| entry.file_name)
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, vec!["game.exe", "tool.EXE"]);
    }

    #[test]
    fn empty_scan_root_is_rejected() {
        let error = normalize_scan_root("   ").expect_err("empty root should fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn backup_copy_and_restore_round_trip_with_unicode_paths() {
        let root = make_test_root();
        let source = root.join("\u{0438}\u{0441}\u{0445}\u{043e}\u{0434}\u{043d}\u{0438}\u{043a}");
        let backup = root.join(
            "\u{0440}\u{0435}\u{0437}\u{0435}\u{0440}\u{0432}\u{043d}\u{0430}\u{044f} \u{043a}\u{043e}\u{043f}\u{0438}\u{044f}",
        );
        let restored = root.join(
            "\u{0432}\u{043e}\u{0441}\u{0441}\u{0442}\u{0430}\u{043d}\u{043e}\u{0432}\u{043b}\u{0435}\u{043d}\u{043e}",
        );
        fs::create_dir_all(&source).expect("source dir should be created");
        fs::create_dir_all(&restored).expect("restore dir should be created");
        let original = source.join(
            "\u{0441}\u{043e}\u{0445}\u{0440}\u{0430}\u{043d}\u{0435}\u{043d}\u{0438}\u{0435}.bin",
        );
        let restore_target = restored.join("\u{4fdd}\u{5b58}.bin");
        fs::write(&original, b"save-data").expect("source file should be written");

        let request_path = root.join("copy-request.jsonl");
        let root_label = "\u{043a}\u{043e}\u{0440}\u{0435}\u{043d}\u{044c}-0";
        let relative_path = "\u{0441}\u{043b}\u{043e}\u{0442} 1/\u{0441}\u{043e}\u{0445}\u{0440}\u{0430}\u{043d}\u{0435}\u{043d}\u{0438}\u{0435}.bin";
        let header = serde_json::to_string(&serde_json::json!({
            "destination": backup.to_string_lossy().to_string(),
        }))
        .expect("copy header should serialize");
        let file = serde_json::to_string(&serde_json::json!({
            "path": original.to_string_lossy().to_string(),
            "rootLabel": root_label,
            "relativePath": relative_path,
            "size": 9,
        }))
        .expect("copy file should serialize");
        fs::write(&request_path, format!("{header}\n{file}\n")).expect("request should be written");

        let (destination, files) = read_copy_request(request_path.to_str().expect("utf8 request"))
            .expect("copy request should parse");
        assert_eq!(destination, backup);
        assert_eq!(files[0].root_label, root_label);
        assert_eq!(files[0].relative_path, relative_path);

        let total = copy_backup_directory(request_path.to_str().expect("utf8 request"))
            .expect("copy should succeed");
        assert_eq!(total, 9);

        let manifest_path = backup.join("__sqoba_manifest.json");
        let manifest = fs::read_to_string(&manifest_path).expect("manifest should exist");
        let mut manifest: BackupManifest =
            serde_json::from_str(&manifest).expect("manifest should parse");
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.files[0].original_path, original.to_string_lossy());
        assert_eq!(
            manifest.files[0].backup_path,
            format!("files/{root_label}/{relative_path}"),
        );
        manifest.files[0].original_path = restore_target.to_string_lossy().to_string();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be rewritten");

        restore_backup_directory(backup.to_str().expect("utf8 backup"))
            .expect("restore should succeed");
        assert_eq!(
            fs::read(&restore_target).expect("restored file should exist"),
            b"save-data",
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn parse_manifest_preserves_unicode_escaped_paths_and_null_mtime() {
        let entries = parse_backup_manifest_entries(
            r#"{"version":2,"files":[{"backupPath":"files/\u043a\u043e\u0440\u0435\u043d\u044c/\u0441\u043b\u043e\u0442.json","originalPath":"C:\\Users\\Kazui\\\u4fdd\u5b58\\save.dat","size":4,"mtime":null}]}"#,
        )
        .expect("manifest should parse");

        assert_eq!(
            entries,
            vec![BackupManifestEntry {
                backup_path: "files/\u{043a}\u{043e}\u{0440}\u{0435}\u{043d}\u{044c}/\u{0441}\u{043b}\u{043e}\u{0442}.json".to_string(),
                original_path: "C:\\Users\\Kazui\\\u{4fdd}\u{5b58}\\save.dat".to_string(),
                size: 4,
                mtime: None,
            }],
        );
    }
}
