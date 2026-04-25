use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ExeEntry {
    pub(crate) path: String,
    pub(crate) file_name: String,
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

pub(crate) fn scan_executables<F>(root: &str, mut on_entry: F) -> io::Result<usize>
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
}
