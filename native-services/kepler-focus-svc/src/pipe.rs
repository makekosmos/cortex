//! Named pipe accept loop. Слушает `\\.\pipe\kosmos-system-service`, на каждое
//! connection — spawn thread, читает один JSON request, выполняет op,
//! пишет JSON response, закрывает pipe.
//!
//! ACL: NULL DACL (= доступно всем user-mode процессам на машине). Acceptable
//! security trade-off для personal app, см. CLAUDE.md per-app constraints.

use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use kepler_focus_svc::protocol;

const PIPE_NAME: &str = r"\\.\pipe\kosmos-system-service";
const BUF_SIZE: u32 = 64 * 1024;
// "D:(A;;GA;;;AU)" → DACL: Allow GenericAll к группе Authenticated Users.
// Это минимальный SDDL, при котором non-elevated user-mode процессы могут
// открыть pipe на read/write. Без SDDL дефолт-ACL ограничивает access по
// owner — а наш service владелец LocalSystem.
const PIPE_SDDL: &str = "D:(A;;GA;;;AU)";

fn hosts_path_for_dispatch() -> PathBuf {
    if let Ok(p) = std::env::var("KEPLER_FOCUS_HOSTS_PATH") {
        return PathBuf::from(p);
    }
    let sysroot = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    PathBuf::from(sysroot).join("System32\\drivers\\etc\\hosts")
}

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Создаёт SECURITY_ATTRIBUTES с DACL из SDDL_STRING. Возвращает SA + handle
/// на SECURITY_DESCRIPTOR memory (нужно LocalFree освободить — но мы держим
/// его до конца программы, leak'ом не страдаем).
unsafe fn build_security_attributes() -> Option<(SECURITY_ATTRIBUTES, PSECURITY_DESCRIPTOR)> {
    let sddl = wide(PIPE_SDDL);
    let mut sd: PSECURITY_DESCRIPTOR = ptr::null_mut();
    let ok = ConvertStringSecurityDescriptorToSecurityDescriptorW(
        sddl.as_ptr(),
        SDDL_REVISION_1 as u32,
        &mut sd,
        ptr::null_mut(),
    );
    if ok == 0 {
        return None;
    }
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd,
        bInheritHandle: 0,
    };
    Some((sa, sd))
}

unsafe fn create_pipe_instance(sa: *const SECURITY_ATTRIBUTES) -> HANDLE {
    let name = wide(PIPE_NAME);
    CreateNamedPipeW(
        name.as_ptr(),
        PIPE_ACCESS_DUPLEX,
        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
        PIPE_UNLIMITED_INSTANCES,
        BUF_SIZE,
        BUF_SIZE,
        0,
        sa as *mut _,
    )
}

/// Главный accept loop. Создаёт новую pipe instance per connection, передаёт
/// её в worker thread. Завершается когда `stop_flag` = true (проверяется
/// между connection'ами).
pub fn accept_loop(stop_flag: Arc<AtomicBool>) {
    let sa_pair = unsafe { build_security_attributes() };
    let sa_ptr: *const SECURITY_ATTRIBUTES = match &sa_pair {
        Some((sa, _)) => sa,
        None => ptr::null(),
    };

    while !stop_flag.load(Ordering::SeqCst) {
        let pipe = unsafe { create_pipe_instance(sa_ptr) };
        if pipe == INVALID_HANDLE_VALUE {
            // Не паникуем — печатаем и продолжаем (с задержкой чтобы не
            // спамить если ошибка постоянна).
            let err = unsafe { GetLastError() };
            eprintln!("CreateNamedPipeW failed: {err}");
            std::thread::sleep(std::time::Duration::from_millis(500));
            continue;
        }

        // ConnectNamedPipe — блокирующий вызов (PIPE_WAIT). Stop пока сюда
        // не пробьётся напрямую; при service stop мы оставляем pending pipe
        // — он закроется когда процесс завершится. Soft acceptable: stop
        // приходит редко, OS чистит handles.
        let connected = unsafe { ConnectNamedPipe(pipe, ptr::null_mut()) };
        if connected == 0 {
            let err = unsafe { GetLastError() };
            if err != ERROR_PIPE_CONNECTED {
                eprintln!("ConnectNamedPipe failed: {err}");
                unsafe { CloseHandle(pipe) };
                continue;
            }
        }

        if stop_flag.load(Ordering::SeqCst) {
            unsafe {
                DisconnectNamedPipe(pipe);
                CloseHandle(pipe);
            }
            break;
        }

        // Hand off to worker thread.
        let h = PipeHandle(pipe);
        thread::spawn(move || {
            handle_connection(h);
        });
    }

    let _ = sa_pair; // hold SD memory until accept_loop exits
}

/// Newtype чтобы handle перешёл во владение thread'а.
struct PipeHandle(HANDLE);

unsafe impl Send for PipeHandle {}

fn handle_connection(pipe: PipeHandle) {
    let mut file = unsafe { std::fs::File::from_raw_handle(pipe.0 as _) };
    let mut raw = String::new();
    {
        let mut reader = BufReader::new(&mut file);
        let _ = reader.read_line(&mut raw);
    }
    let resp = protocol::handle_raw(&raw, &hosts_path_for_dispatch());
    let json = serde_json::to_string(&resp)
        .unwrap_or_else(|_| String::from(r#"{"ok":false,"error":"serialize failed"}"#));
    let _ = writeln!(file, "{json}");
    let _ = file.flush();
    // Drop file → CloseHandle.
    let _ = (ERROR_IO_PENDING, FILE_FLAG_OVERLAPPED); // referenced to keep imports stable across feature combos
}
