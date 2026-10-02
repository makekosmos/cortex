//! Process handling for the installer subcommands (KOS-306).
//!
//! Replaces the fifteen `taskkill /F /IM …` spawns the NSIS script used to
//! run, and the `Get-CimInstance Win32_Process` lookups the PowerShell
//! installer did. Enumeration goes through the ToolHelp snapshot API;
//! matching a process to an exe path uses `QueryFullProcessImageNameW` —
//! the same semantics as `Win32_Process.ExecutablePath`.
//!
//! KOS-309: a kill is only done when the process is *gone* — the installer's
//! next step renames directories the dying process still holds handles into.
//! `TerminateProcess` merely requests the exit, so every kill waits on the
//! process handle, and `kill-product-processes` exits non-zero when any
//! process survives.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use serde::Serialize;

/// Bound on waiting for a terminated (or gracefully asked) process to exit.
/// Process teardown — dll unload, handle release — still runs after
/// TerminateProcess / `--shutdown` returns, and the installer's rename step
/// needs those handles closed. Ten seconds is generous for teardown yet
/// short enough that a wedged process fails the install loudly instead of
/// hanging it.
const PROCESS_EXIT_TIMEOUT: Duration = Duration::from_secs(10);

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

/// whisper-server.exe — the local-STT sidecar the Engine spawns from
/// `%APPDATA%\Mundus\tools\dictation\whisper.cpp*\Release\`. It outlives the
/// Engine kill as an orphan and keeps its tools dir pinned. It is matched by
/// name AND by image path: another application may ship a whisper-server
/// too, so the bare name must never be killed.
const WHISPER_SERVER_NAME: &str = "whisper-server.exe";

/// One image name to kill, optionally constrained to processes whose exe
/// path is under `under` (see [`WHISPER_SERVER_NAME`]).
pub(crate) struct KillTarget<'a> {
    pub name: &'a str,
    pub under: Option<PathBuf>,
}

/// The full target list for `kill-product-processes`: every product name
/// unconditionally, plus whisper-server scoped to our own tools dirs.
/// `local_stt_roots` mirrors `dictation::local_models::shared_assets_root`:
/// `%APPDATA%\Mundus` by default, overridden by `MUNDUS_LOCAL_STT_DIR`.
fn kill_targets() -> Vec<KillTarget<'static>> {
    let mut targets: Vec<KillTarget> = PRODUCT_PROCESS_NAMES
        .iter()
        .map(|name| KillTarget { name, under: None })
        .collect();
    let mut roots = Vec::new();
    if let Ok(dir) = std::env::var("MUNDUS_LOCAL_STT_DIR") {
        if !dir.trim().is_empty() {
            roots.push(PathBuf::from(dir.trim()));
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        roots.push(PathBuf::from(appdata).join("Mundus"));
    }
    for root in roots {
        targets.push(KillTarget {
            name: WHISPER_SERVER_NAME,
            under: Some(root.join("tools").join("dictation")),
        });
    }
    targets
}

/// What the terminate-and-wait step observed for one process.
#[cfg(windows)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum KillOutcome {
    /// The process exited (or was already gone).
    Gone,
    /// Still alive after [`PROCESS_EXIT_TIMEOUT`].
    Survived,
}

#[derive(Debug, Default, Serialize)]
pub struct KillReport {
    pub killed: Vec<String>,
    /// Terminate was requested but the process was still alive when the wait
    /// timed out — the install must abort, these still pin files.
    pub survived: Vec<String>,
    pub failed: Vec<String>,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
        PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        PROCESS_TERMINATE,
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

    /// pids of live processes whose image name is one of `names` and whose
    /// exe path is exactly `path`. Errors when a matching-named process'
    /// path cannot be determined — silently replacing a file a live Engine
    /// has mapped is how corrupt installs happen.
    fn pids_at(path: &Path, names: &[&str]) -> Result<Vec<u32>, String> {
        let target = normalize(path);
        let mut pids = Vec::new();
        for (pid, image) in snapshot()? {
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&image)) {
                continue;
            }
            let Some(running) = exe_path(pid) else {
                return Err("cannot determine the running Engine path".into());
            };
            if normalize(&running).eq_ignore_ascii_case(&target) {
                pids.push(pid);
            }
        }
        Ok(pids)
    }

    /// True while a process whose image name is one of `names` runs from
    /// exactly `path`.
    pub fn is_running_at(path: &Path, names: &[&str]) -> Result<bool, String> {
        Ok(!pids_at(path, names)?.is_empty())
    }

    fn normalize(path: &Path) -> String {
        // canonicalize resolves junctions/short names; keep a plain
        // absolute fallback for paths that no longer exist on disk.
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        path.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_owned()
    }

    /// `path` is inside `dir` (case-insensitive, after junction/8.3
    /// normalization). Used to scope whisper-server kills to our own tools
    /// dir — a same-named exe elsewhere must survive.
    pub(crate) fn is_under(path: &Path, dir: &Path) -> bool {
        let path = normalize(path).to_lowercase();
        let dir = normalize(dir).to_lowercase();
        path.starts_with(&format!("{dir}\\"))
    }

    /// Terminate `pid` and wait on the process handle until the kernel
    /// reports it gone or [`PROCESS_EXIT_TIMEOUT`] elapses.
    pub(crate) fn terminate_and_wait(pid: u32) -> Result<KillOutcome, String> {
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid)
                .map_err(|e| format!("OpenProcess: {e}"))?;
            let result = (|| {
                TerminateProcess(handle, 1).map_err(|e| format!("TerminateProcess: {e}"))?;
                let event = WaitForSingleObject(handle, PROCESS_EXIT_TIMEOUT.as_millis() as u32);
                if event == WAIT_OBJECT_0 {
                    Ok(KillOutcome::Gone)
                } else if event == WAIT_TIMEOUT {
                    Ok(KillOutcome::Survived)
                } else {
                    Err(format!("WaitForSingleObject returned {event:?}"))
                }
            })();
            let _ = CloseHandle(handle);
            result
        }
    }

    /// Kill every process matching one of `targets`; the calling process'
    /// own pid is always excluded (the staged exe is itself
    /// `mundus-engine.exe` during an install). `kill` is the terminate step,
    /// injected so tests can exercise the survivor path without a real
    /// unkillable process.
    pub(crate) fn kill_by_targets_with(
        targets: &[KillTarget<'_>],
        kill: &dyn Fn(u32) -> Result<KillOutcome, String>,
    ) -> Result<KillReport, String> {
        let own_pid = std::process::id();
        let mut report = KillReport::default();
        for (pid, image) in snapshot()? {
            if pid == own_pid {
                continue;
            }
            let Some(target) = targets.iter().find(|t| t.name.eq_ignore_ascii_case(&image)) else {
                continue;
            };
            if let Some(dir) = &target.under {
                match exe_path(pid) {
                    Some(running) if is_under(&running, dir) => {}
                    // Same name but a foreign install location — not ours.
                    Some(_) => continue,
                    // Path unreadable: we cannot prove it is ours to kill,
                    // but if it is ours it still pins files — report, don't
                    // kill, don't pretend it's gone.
                    None => {
                        report.failed.push(format!(
                            "{image} (pid {pid}): image path unreadable — cannot prove it is ours"
                        ));
                        continue;
                    }
                }
            }
            match kill(pid) {
                Ok(KillOutcome::Gone) => report.killed.push(image),
                Ok(KillOutcome::Survived) => {
                    report.survived.push(format!("{image} (pid {pid})"));
                }
                Err(e) => {
                    report.failed.push(format!("{image} (pid {pid}): {e}"));
                }
            }
        }
        Ok(report)
    }

    /// Kill every process whose image name is in `names`. Returns the
    /// per-name result.
    pub fn kill_by_names(names: &[&str]) -> Result<KillReport, String> {
        let targets: Vec<KillTarget> = names
            .iter()
            .map(|name| KillTarget { name, under: None })
            .collect();
        kill_by_targets_with(&targets, &terminate_and_wait)
    }

    /// All `kill-product-processes` targets: product names plus the
    /// path-scoped whisper-server.
    pub fn kill_product_targets() -> Result<KillReport, String> {
        kill_by_targets_with(&kill_targets(), &terminate_and_wait)
    }

    /// Graceful stop: `<exe> --shutdown` asks the running Engine to exit on
    /// its own control path, then we wait on the process handles until the
    /// kernel reports them gone. Returns false when nothing was running.
    /// Errors when a process survives — the installer must not replace files
    /// a live process still has mapped.
    pub fn stop_for_replacement(exe: &Path) -> Result<bool, String> {
        let pids = pids_at(exe, ENGINE_BINARY_NAMES)?;
        if pids.is_empty() {
            return Ok(false);
        }
        // Open SYNCHRONIZE handles before asking for shutdown: the wait then
        // observes the very processes we found, not a recycled pid.
        let mut handles = Vec::new();
        for &pid in &pids {
            // A failed open means the process went away between snapshot and
            // open — the desired state, so it is simply not waited on.
            unsafe {
                if let Ok(handle) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
                    handles.push((pid, handle));
                }
            }
        }
        if handles.is_empty() {
            return Ok(true);
        }
        let spawn = std::process::Command::new(exe)
            .arg("--shutdown")
            .status()
            .map_err(|e| format!("spawn {exe:?} --shutdown: {e}"));
        let mut survivors = Vec::new();
        for (pid, handle) in &handles {
            unsafe {
                let event = WaitForSingleObject(*handle, PROCESS_EXIT_TIMEOUT.as_millis() as u32);
                if event != WAIT_OBJECT_0 {
                    survivors.push(format!("pid {pid}"));
                }
                let _ = CloseHandle(*handle);
            }
        }
        spawn?;
        if !survivors.is_empty() {
            return Err(format!(
                "running Engine did not stop before replacement: {}",
                survivors.join(", ")
            ));
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
    pub fn kill_product_targets() -> Result<KillReport, String> {
        Ok(KillReport::default())
    }
    pub fn stop_for_replacement(_exe: &Path) -> Result<bool, String> {
        Ok(false)
    }
}

pub(crate) use imp::*;

/// `true` when nothing survived and nothing failed — the exit-code mapping
/// of the subcommand, kept in one place so tests cover the real condition.
pub(crate) fn report_ok(report: &KillReport) -> bool {
    report.survived.is_empty() && report.failed.is_empty()
}

/// `mundus-engine kill-product-processes` — the single mop-up the installer
/// and uninstaller run after the graceful `--shutdown`. One process spawn,
/// no `taskkill`, and only our own product image names (plus a path-scoped
/// whisper-server) are ever touched. Non-zero when anything is left alive —
/// the installer aborts on that instead of renaming over pinned files.
pub fn run_kill_product_processes() -> ExitCode {
    match kill_product_targets() {
        Ok(report) => {
            println!("{}", serde_json::json!({ "ok": true, "report": report }));
            if report_ok(&report) {
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
