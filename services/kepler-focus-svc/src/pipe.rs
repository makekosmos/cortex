//! Named pipe accept loop. Слушает `\\.\pipe\kepler-focus-svc`, на каждое
//! connection — spawn thread, читает один JSON request, выполняет op,
//! пишет JSON response, закрывает pipe.
//!
//! ACL: NULL DACL (= доступно всем user-mode процессам на машине). Acceptable
//! security trade-off для personal app, см. CLAUDE.md per-app constraints.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
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
use windows_sys::Win32::Storage::FileSystem::{
    ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use kepler_focus_svc::protocol;

const PIPE_NAME: &str = r"\\.\pipe\kepler-focus-svc";
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
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
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

/// Newtype чтобы handle перешёл во владение thread'а и Drop закрыл его.
struct PipeHandle(HANDLE);

unsafe impl Send for PipeHandle {}

impl Drop for PipeHandle {
    fn drop(&mut self) {
        unsafe {
            DisconnectNamedPipe(self.0);
            CloseHandle(self.0);
        }
    }
}

fn handle_connection(pipe: PipeHandle) {
    let mut buf = vec![0u8; BUF_SIZE as usize];
    let mut total = 0usize;

    // Простой read loop: читаем до EOF клиента (он закрывает write-side
    // после single message) или до полного буфера. Клиенту достаточно сделать
    // shutdown writes (или close) после write — ReadFile тогда вернёт 0.
    loop {
        let mut read: u32 = 0;
        let ok = unsafe {
            ReadFile(
                pipe.0,
                buf.as_mut_ptr().add(total) as *mut _,
                (buf.len() - total) as u32,
                &mut read,
                ptr::null_mut(),
            )
        };
        if ok == 0 || read == 0 {
            break;
        }
        total += read as usize;
        if total >= buf.len() {
            break;
        }
        // Эвристика: если в накопленном буфере есть валидный JSON terminator
        // (newline) — прекращаем чтение. Большинство клиентов пошлёт single
        // line.
        if buf[..total].contains(&b'\n') {
            break;
        }
    }

    let raw = String::from_utf8_lossy(&buf[..total]);
    let resp = protocol::handle_raw(&raw, &hosts_path_for_dispatch());
    let json = serde_json::to_string(&resp)
        .unwrap_or_else(|_| String::from(r#"{"ok":false,"error":"serialize failed"}"#));
    let mut payload = json.into_bytes();
    payload.push(b'\n');

    let mut written: u32 = 0;
    let mut offset = 0usize;
    while offset < payload.len() {
        let ok = unsafe {
            WriteFile(
                pipe.0,
                payload.as_ptr().add(offset) as *const _,
                (payload.len() - offset) as u32,
                &mut written,
                ptr::null_mut(),
            )
        };
        if ok == 0 || written == 0 {
            break;
        }
        offset += written as usize;
    }
    // Drop pipe → DisconnectNamedPipe + CloseHandle.
    let _ = (ERROR_IO_PENDING, FILE_FLAG_OVERLAPPED); // referenced to keep imports stable across feature combos
}
