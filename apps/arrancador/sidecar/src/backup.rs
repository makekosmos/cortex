use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::protocol::write_backup_progress;

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

fn ensure_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

fn validate_backup_segment(kind: &str, value: &str) -> io::Result<()> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains(':')
        || value.contains('/')
        || value.contains('\\')
        || Path::new(value).is_absolute()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid backup {kind}: {value}"),
        ));
    }
    Ok(())
}

fn normalize_backup_relative_path(relative_path: &str) -> io::Result<String> {
    let normalized = relative_path.replace('\\', "/");
    if normalized.starts_with('/') || normalized.starts_with('\\') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid backup relative path: {relative_path}"),
        ));
    }
    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid backup relative path: {relative_path}"),
        ));
    }
    if normalized.is_empty() {
        return Ok(String::new());
    }
    for segment in normalized.split('/') {
        validate_backup_segment("relative path segment", segment)?;
    }
    Ok(normalized)
}

fn build_backup_rel_path(root_label: &str, relative_path: &str) -> io::Result<String> {
    validate_backup_segment("root label", root_label)?;
    let rel = normalize_backup_relative_path(relative_path)?;
    if rel.is_empty() {
        return Ok(format!("files/{}/file", root_label));
    }
    Ok(format!("files/{}/{}", root_label, rel))
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

pub(crate) fn copy_backup_directory(request_path: &str) -> io::Result<u64> {
    let (destination, files) = read_copy_request(request_path)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    ensure_dir(&destination)?;
    let total = files.len();
    let mut total_bytes = 0;
    let mut entries = Vec::with_capacity(files.len());

    for (index, file) in files.iter().enumerate() {
        let backup_path = build_backup_rel_path(&file.root_label, &file.relative_path)?;
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
    let normalized = rel.replace('\\', "/");
    if normalized.starts_with('/') || normalized.starts_with('\\') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid backup path in manifest: {rel}"),
        ));
    }
    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid backup path in manifest: {rel}"),
        ));
    }
    if normalized.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid backup path in manifest: {rel}"),
        ));
    }
    for part in normalized.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains(':')
            || Path::new(part).is_absolute()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid backup path in manifest: {rel}"),
            ));
        }
    }
    Ok(())
}

fn normalize_restore_root(root: &str) -> String {
    root
        .replace('/', "\\")
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
}

fn validate_allowed_restore_roots(roots: &[String]) -> io::Result<Vec<String>> {
    let normalized: Vec<String> = roots
        .iter()
        .map(|root| normalize_restore_root(root))
        .filter(|root| !root.is_empty())
        .collect();
    if normalized.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Restore target roots are required",
        ));
    }
    Ok(normalized)
}

fn assert_within_restore_roots(target: &Path, allowed_roots: &[String]) -> io::Result<()> {
    let normalized_target = normalize_restore_root(&target.to_string_lossy());
    let is_allowed = allowed_roots.iter().any(|root| {
        normalized_target == *root || normalized_target.starts_with(&format!("{root}\\"))
    });
    if !is_allowed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Restore target is outside allowed roots: {}",
                target.to_string_lossy()
            ),
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

pub(crate) fn restore_backup_directory(
    backup_root: &str,
    allowed_restore_roots: &[String],
) -> io::Result<()> {
    let backup_root = PathBuf::from(backup_root);
    let allowed_restore_roots = validate_allowed_restore_roots(allowed_restore_roots)?;
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
        assert_within_restore_roots(&target, &allowed_restore_roots)?;
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
        assert_eq!(manifest.files[0].backup_path, format!("files/{root_label}/{relative_path}"));
        manifest.files[0].original_path = restore_target.to_string_lossy().to_string();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be rewritten");

        let allowed_roots = vec![restored.to_string_lossy().to_string()];
        restore_backup_directory(backup.to_str().expect("utf8 backup"), &allowed_roots)
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

    #[test]
    fn backup_copy_rejects_unsafe_relative_paths() {
        for (root_label, relative_path) in [
            ("root-0", "../save.dat"),
            ("root-0", "slot/../save.dat"),
            ("root-0", "/absolute/save.dat"),
            ("root-0", "C:/Users/save.dat"),
            ("../root", "save.dat"),
        ] {
            assert!(
                build_backup_rel_path(root_label, relative_path).is_err(),
                "{root_label} + {relative_path} should be rejected",
            );
        }
    }

    #[test]
    fn backup_restore_rejects_unsafe_manifest_paths() {
        let root = make_test_root();
        let backup = root.join("backup");
        fs::create_dir_all(&backup).expect("backup dir should be created");
        let restore_target = root.join("restore").join("save.dat");
        let manifest_path = backup.join("__sqoba_manifest.json");

        for backup_path in [
            "../files/root/save.dat",
            "files/root/../save.dat",
            "files/root//save.dat",
            "files/root/save.dat/",
            "/absolute/save.dat",
            "C:/files/root/save.dat",
            "\\\\server\\share\\save.dat",
        ] {
            let manifest = serde_json::to_string_pretty(&BackupManifest {
                version: 2,
                files: vec![BackupManifestEntry {
                    backup_path: backup_path.to_string(),
                    original_path: restore_target.to_string_lossy().to_string(),
                    size: 9,
                    mtime: None,
                }],
            })
            .expect("manifest should serialize");
            fs::write(&manifest_path, manifest).expect("manifest should be written");

            let allowed_roots = vec![root.to_string_lossy().to_string()];
            let error = restore_backup_directory(backup.to_str().expect("utf8 backup"), &allowed_roots)
                .expect_err("unsafe manifest path should be rejected");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(
                error
                    .to_string()
                    .contains("Invalid backup path in manifest"),
                "{backup_path} should fail as an invalid manifest path, got {error}",
            );
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn backup_restore_rejects_targets_outside_allowed_roots() {
        let root = make_test_root();
        let backup = root.join("backup");
        let allowed = root.join("allowed");
        let outside = root.join("outside").join("save.dat");
        fs::create_dir_all(backup.join("files/root-0")).expect("backup dir should be created");
        fs::write(backup.join("files/root-0/save.dat"), b"save-data")
            .expect("backup file should be written");

        let manifest = serde_json::to_string_pretty(&BackupManifest {
            version: 2,
            files: vec![BackupManifestEntry {
                backup_path: "files/root-0/save.dat".to_string(),
                original_path: outside.to_string_lossy().to_string(),
                size: 9,
                mtime: None,
            }],
        })
        .expect("manifest should serialize");
        fs::write(backup.join("__sqoba_manifest.json"), manifest)
            .expect("manifest should be written");

        let allowed_roots = vec![allowed.to_string_lossy().to_string()];
        let error = restore_backup_directory(backup.to_str().expect("utf8 backup"), &allowed_roots)
            .expect_err("out-of-root restore target should be rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains("Restore target is outside allowed roots"),
            "unexpected error: {error}",
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn backup_restore_accepts_allowed_root_with_forward_slashes() {
        let root = make_test_root();
        let backup = root.join("backup");
        let allowed = root.join("allowed");
        let restore_target = allowed.join("slot").join("save.dat");
        fs::create_dir_all(backup.join("files/root-0")).expect("backup dir should be created");
        fs::write(backup.join("files/root-0/save.dat"), b"save-data")
            .expect("backup file should be written");

        let manifest = serde_json::to_string_pretty(&BackupManifest {
            version: 2,
            files: vec![BackupManifestEntry {
                backup_path: "files/root-0/save.dat".to_string(),
                original_path: restore_target.to_string_lossy().to_string(),
                size: 9,
                mtime: None,
            }],
        })
        .expect("manifest should serialize");
        fs::write(backup.join("__sqoba_manifest.json"), manifest)
            .expect("manifest should be written");

        let allowed_roots = vec![allowed.to_string_lossy().replace('\\', "/")];
        restore_backup_directory(backup.to_str().expect("utf8 backup"), &allowed_roots)
            .expect("restore target under slash-normalized root should be accepted");
        assert_eq!(
            fs::read(&restore_target).expect("restored file should exist"),
            b"save-data",
        );

        let _ = fs::remove_dir_all(&root);
    }
}
