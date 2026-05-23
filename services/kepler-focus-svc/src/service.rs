//! ServiceMain entry — регистрируется в SCM, поднимает pipe accept loop в
//! отдельном thread, ждёт ServiceControl::Stop, выставляет Stopped.

use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use windows_service::define_windows_service;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;

use crate::cli::SERVICE_NAME;
use crate::pipe;

define_windows_service!(ffi_service_main, service_main);

/// User-mode entry. Передаёт control SCM который вызовет `service_main`.
/// Возвращает только когда SCM решит завершить процесс.
pub fn run_as_service_entry() -> ! {
    if let Err(e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        eprintln!("service_dispatcher::start failed: {e}");
        std::process::exit(1);
    }
    std::process::exit(0);
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

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

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
        pipe::accept_loop(stop_flag_pipe);
    });

    // Ждём команду stop. recv блокирует до Stop/Shutdown.
    let _ = shutdown_rx.recv();
    stop_flag.store(true, Ordering::SeqCst);

    // Pipe thread may be blocked in ConnectNamedPipe. Do not join it during
    // SCM stop; process exit closes the pipe handle.

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
