//! ServiceMain entry — registers with the SCM, runs the pipe accept loop on
//! a worker thread, waits for ServiceControl::Stop/Shutdown, reports Stopped.
//!
//! The service process is the copied Engine binary running
//! `privileged run-service --grant-sid <SID>`. It never executes anything
//! from a user-writable path: the binary itself lives under
//! `%ProgramFiles%\<Brand>\Service` and every op it serves is a fixed
//! `protocol::Request` variant.

#![cfg(windows)]

use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use windows_service::define_windows_service;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;

use crate::privileged::brand;
use crate::privileged::pipe_server;

define_windows_service!(ffi_service_main, service_main);

/// The enabling user's SID — captured by `run_service_entry` from the
/// `--grant-sid` launch argument and read by the accept loop for the pipe DACL.
static GRANT_SID: Mutex<Option<String>> = Mutex::new(None);

/// User-mode entry — hands control to the SCM, which calls `service_main`.
/// Returns only when the SCM decides to terminate the process.
pub fn run_service_entry(grant_sid: String) -> ! {
    if let Ok(mut guard) = GRANT_SID.lock() {
        *guard = Some(grant_sid);
    }
    match service_dispatcher::start(brand::SERVICE_NAME, ffi_service_main) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("service_dispatcher::start failed: {e}");
            std::process::exit(1);
        }
    }
}

fn grant_sid() -> String {
    GRANT_SID
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .unwrap_or_default()
}

fn service_main(_args: Vec<OsString>) {
    if let Err(e) = run_service() {
        eprintln!("service run failed: {e}");
    }
}

fn run_service() -> windows_service::Result<()> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_handler = Arc::clone(&stop_flag);

    let event_handler = move |control| -> ServiceControlHandlerResult {
        match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                stop_flag_handler.store(true, Ordering::SeqCst);
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(brand::SERVICE_NAME, event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    let stop_flag_pipe = Arc::clone(&stop_flag);
    let _pipe_thread = thread::spawn(move || {
        pipe_server::accept_loop(stop_flag_pipe, &grant_sid());
    });

    // Block until Stop/Shutdown. The pipe worker may sit in ConnectNamedPipe
    // when we exit — process teardown closes the handle.
    let _ = shutdown_rx.recv();
    stop_flag.store(true, Ordering::SeqCst);

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}
