//! Process handling for the installer subcommands (KOS-306).
//!
//! Replaces the fifteen `taskkill /F /IM …` spawns the NSIS script used to
//! run, and the `Get-CimInstance Win32_Process` lookups the PowerShell
//! installer did. Enumeration goes through the ToolHelp snapshot API;
//! matching a process to an exe path uses `QueryFullProcessImageNameW` —
//! the same semantics as `Win32_Process.ExecutablePath`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread::sleep;
use std::time::Duration;

use serde::Serialize;

/// Engine binary file names from every product generation, oldest first.
/// MIGRATION(KOS-267): drop the legacy names after 2026-11-01.
pub(crate) const ENGINE_BINARY_NAMES: &[&str] = &[
    "mundus-engine.exe",
    "kepler-backend.exe", // MIGRATION(KOS-267)
];

/// Every image name the installer kills before touching files: the current
/// product plus the ones a 0.9.x/Electron-era install may have left running.
/// MIGRATION(KOS-267): remove the legacy names after 2026-11-01.
const PRODUCT_PROCESS_NAMES: &[&str] = &[
    "Mundus.exe",
    "mundus-engine.exe",
    "Mundus Manager.exe",
    // Store-installed native apps (KOS-265) — must not hold their dir while
    // the Engine or an uninstall replaces/removes it.
    "agenda-gpui.exe",
    "memoria-gpui.exe",
    "dictation-gpui.exe",
    "Agenda.exe",    // MIGRATION(KOS-267)
    "Memoria.exe",   // MIGRATION(KOS-267)
    "Dictation.exe", // MIGRATION(KOS-267)
    // 0.9.x installs leave an orphaned ark-core-rpc.exe child holding the DB.
    "ark-core-rpc.exe",     // MIGRATION(KOS-267)
    "Kosmos.exe",           // MIGRATION(KOS-267)
    "kepler-backend.exe",   // MIGRATION(KOS-267)
    "Kosmos Manager.exe",   // MIGRATION(KOS-267)
    "Kosmos Agenda.exe",    // MIGRATION(KOS-267)
    "Kosmos Memoria.exe",   // MIGRATION(KOS-267)
    "Kosmos Dictation.exe", // MIGRATION(KOS-267)
];

#[derive(Debug, Default, Serialize)]
pub struct KillReport {
    pub killed: Vec<String>,
    pub failed: Vec<String>,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    };

    /// `(pid, image file name)` for every process in the system snapshot.
    fn snapshot() -> Result<Vec<(u32, String)>, String> {
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .map_err(|e| format!("CreateToolhelp32Snapshot: {e}"))?;
            let mut entries = Vec::new();
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut next = Process32FirstW(snap, &mut entry);
            while next.is_ok() {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                entries.push((
                    entry.th32ProcessID,
                    String::from_utf16_lossy(&entry.szExeFile[..len]),
                ));
                next = Process32NextW(snap, &mut entry);
            }
            let _ = CloseHandle(snap);
            Ok(entries)
        }
    }

    /// Full image path of `pid`, or None when the process already exited or
    /// refuses a query (access denied on a system process is not our
    /// business — we only ever match our own image names).
    fn exe_path(pid: u32) -> Option<PathBuf> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buffer = vec![0u16; 32768];
            let mut len = buffer.len() as u32;
            let result = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buffer.as_mut_ptr()),
                &mut len,
            );
            let _ = CloseHandle(handle);
            result.ok()?;
            Some(PathBuf::from(String::from_utf16_lossy(
                &buffer[..len as usize],
            )))
        }
    }

    /// True while a process whose image name is one of `names` runs from
    /// exactly `path`. Errors when a matching-named process' path cannot be
    /// determined — silently replacing a file a live Engine has mapped is
    /// how corrupt installs happen.
    pub fn is_running_at(path: &Path, names: &[&str]) -> Result<bool, String> {
        let target = normalize(path);
        for (pid, image) in snapshot()? {
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&image)) {
                continue;
            }
            let Some(running) = exe_path(pid) else {
                return Err("cannot determine the running Engine path".into());
            };
            if normalize(&running).eq_ignore_ascii_case(&target) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn normalize(path: &Path) -> String {
        // canonicalize resolves junctions/short names; keep a plain
        // absolute fallback for paths that no longer exist on disk.
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        path.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_owned()
    }

    /// Kill every process whose image name is in `names`; the calling
    /// process' own pid is always excluded (the staged exe is itself
    /// `mundus-engine.exe` during an install). Returns the per-name result.
    pub fn kill_by_names(names: &[&str]) -> Result<KillReport, String> {
        let own_pid = std::process::id();
        let mut report = KillReport::default();
        for (pid, image) in snapshot()? {
            if pid == own_pid || !names.iter().any(|n| n.eq_ignore_ascii_case(&image)) {
                continue;
            }
            match terminate(pid) {
                Ok(()) => report.killed.push(image),
                Err(e) => {
                    report.failed.push(format!("{image} (pid {pid}): {e}"));
                }
            }
        }
        Ok(report)
    }

    fn terminate(pid: u32) -> Result<(), String> {
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, false, pid)
                .map_err(|e| format!("OpenProcess: {e}"))?;
            let result = TerminateProcess(handle, 1);
            let _ = CloseHandle(handle);
            result.map_err(|e| format!("TerminateProcess: {e}"))
        }
    }

    /// Graceful stop: `<exe> --shutdown` asks the running Engine to exit on
    /// its own control path, then we poll until it is gone. Returns false
    /// when nothing was running. Errors when the process survives — the
    /// installer must not replace files a live process still has mapped.
    pub fn stop_for_replacement(exe: &Path) -> Result<bool, String> {
        if !is_running_at(exe, ENGINE_BINARY_NAMES)? {
            return Ok(false);
        }
        std::process::Command::new(exe)
            .arg("--shutdown")
            .status()
            .map_err(|e| format!("spawn {exe:?} --shutdown: {e}"))?;
        sleep(Duration::from_millis(750));
        if is_running_at(exe, ENGINE_BINARY_NAMES)? {
            return Err("running Engine did not stop before replacement".into());
        }
        Ok(true)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn is_running_at(_path: &Path, _names: &[&str]) -> Result<bool, String> {
        Ok(false)
    }
    pub fn kill_by_names(_names: &[&str]) -> Result<KillReport, String> {
        Ok(KillReport::default())
    }
    pub fn stop_for_replacement(_exe: &Path) -> Result<bool, String> {
        Ok(false)
    }
}

pub(crate) use imp::*;

/// `mundus-engine kill-product-processes` — the single mop-up the installer
/// and uninstaller run after the graceful `--shutdown`. One process spawn,
/// no `taskkill`, and only our own product image names are ever touched.
pub fn run_kill_product_processes() -> ExitCode {
    match kill_by_names(PRODUCT_PROCESS_NAMES) {
        Ok(report) => {
            println!("{}", serde_json::json!({ "ok": true, "report": report }));
            if report.failed.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            println!("{}", serde_json::json!({ "ok": false, "error": error }));
            ExitCode::FAILURE
        }
    }
}
