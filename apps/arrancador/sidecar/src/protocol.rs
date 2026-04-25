use std::io::{self, Write};

use serde::{Deserialize, Serialize};

use crate::scan::ExeEntry;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation")]
pub(crate) enum Request {
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
        #[serde(rename = "allowedRestoreRoots")]
        allowed_restore_roots: Vec<String>,
    },
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

fn write_json_line<T: Serialize>(value: &T) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, value)
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    lock.write_all(b"\n")?;
    lock.flush()
}

pub(crate) fn write_scan_entry(entry: &ExeEntry) -> io::Result<()> {
    write_json_line(&ScanEntryEvent {
        event: "scan_entry",
        entry,
    })
}

pub(crate) fn write_success_count(count: usize) -> io::Result<()> {
    write_json_line(&success_response(count))
}

pub(crate) fn write_success_total_bytes(total_bytes: u64) -> io::Result<()> {
    write_json_line(&success_response(BackupCopyData { total_bytes }))
}

pub(crate) fn write_success_bool(value: bool) -> io::Result<()> {
    write_json_line(&success_response(value))
}

pub(crate) fn write_error(error: impl ToString) -> io::Result<()> {
    write_json_line(&error_response(error))
}

pub(crate) fn write_backup_progress(
    stage: &str,
    current: &str,
    done: usize,
    total: usize,
) -> io::Result<()> {
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

pub(crate) fn parse_request(line: &str) -> Result<Request, String> {
    serde_json::from_str(line).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_restore_request_with_allowed_roots() {
        let request = parse_request(
            r#"{"operation":"restore_backup_directory","backupRoot":"D:\\Backups\\b1","allowedRestoreRoots":["C:\\Users\\Kazui\\Saved Games"]}"#,
        )
        .expect("request should parse");

        assert_eq!(
            request,
            Request::RestoreBackupDirectory {
                backup_root: "D:\\Backups\\b1".to_string(),
                allowed_restore_roots: vec!["C:\\Users\\Kazui\\Saved Games".to_string()],
            },
        );
    }
}
