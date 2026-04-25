use std::io::{self, BufRead};

mod backup;
mod protocol;
mod scan;

use backup::{copy_backup_directory, restore_backup_directory};
use protocol::{
    parse_request, write_error, write_scan_entry, write_success_bool, write_success_count,
    write_success_total_bytes, Request,
};
use scan::scan_executables;

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
        Request::RestoreBackupDirectory {
            backup_root,
            allowed_restore_roots,
        } => {
            restore_backup_directory(&backup_root, &allowed_restore_roots)?;
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
