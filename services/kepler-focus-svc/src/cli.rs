//! Sub-commands user-mode entry points: install / uninstall / start / stop /
//! status. Все они говорят со Service Control Manager через `windows-service`
//! crate. Не-admin caller получает needs_elevation=true.

use std::ffi::OsString;
use std::time::Duration;

use windows_service::service::{
    ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceState, ServiceType,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

use crate::CliResponse;

pub const SERVICE_NAME: &str = "KeplerFocusSvc";
pub const SERVICE_DISPLAY_NAME: &str = "Kepler Focus Service";
pub const SERVICE_DESCRIPTION: &str =
    "Hosts file management для focus mode (часть Kepler / Kosmos). \
Можно безопасно удалить: sc stop KeplerFocusSvc && sc delete KeplerFocusSvc.";

fn err(msg: impl Into<String>) -> CliResponse {
    CliResponse {
        ok: false,
        error: Some(msg.into()),
        needs_elevation: None,
        service_name: None,
        installed: None,
        running: None,
    }
}

fn err_elevation(msg: impl Into<String>) -> CliResponse {
    CliResponse {
        ok: false,
        error: Some(msg.into()),
        needs_elevation: Some(true),
        service_name: None,
        installed: None,
        running: None,
    }
}

fn map_open_scm_err(e: windows_service::Error) -> CliResponse {
    let msg = format!("open SCM failed: {e}");
    // Win32 ERROR_ACCESS_DENIED = 5. windows-service оборачивает в io::Error;
    // дешёвый эвристический детект по тексту.
    let lower = msg.to_lowercase();
    if lower.contains("access is denied") || lower.contains("access denied") || lower.contains("os error 5") {
        err_elevation(msg)
    } else {
        err(msg)
    }
}

fn exe_path() -> Result<std::path::PathBuf, CliResponse> {
    std::env::current_exe().map_err(|e| err(format!("current_exe failed: {e}")))
}

pub fn install() -> ! {
    let scm = match ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    ) {
        Ok(s) => s,
        Err(e) => map_open_scm_err(e).print_and_exit(),
    };

    let exe = match exe_path() {
        Ok(p) => p,
        Err(r) => r.print_and_exit(),
    };

    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::OnDemand,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: vec![OsString::from("run-as-service")],
        dependencies: vec![],
        // LocalSystem = None.
        account_name: None,
        account_password: None,
    };

    let svc = match scm.create_service(&info, ServiceAccess::CHANGE_CONFIG) {
        Ok(s) => s,
        Err(e) => err(format!("create_service failed: {e}")).print_and_exit(),
    };

    // Description — best-effort, ignore failure.
    let _ = svc.set_description(SERVICE_DESCRIPTION);

    let resp = CliResponse {
        ok: true,
        error: None,
        needs_elevation: None,
        service_name: Some(SERVICE_NAME.into()),
        installed: Some(true),
        running: None,
    };
    resp.print_and_exit();
}

pub fn uninstall() -> ! {
    let scm = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        Ok(s) => s,
        Err(e) => map_open_scm_err(e).print_and_exit(),
    };

    let svc = match scm.open_service(
        SERVICE_NAME,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
    ) {
        Ok(s) => s,
        Err(e) => {
            // Not installed → idempotent success.
            let msg = format!("{e}");
            if msg.to_lowercase().contains("does not exist")
                || msg.to_lowercase().contains("1060")
            {
                let resp = CliResponse {
                    ok: true,
                    error: None,
                    needs_elevation: None,
                    service_name: Some(SERVICE_NAME.into()),
                    installed: Some(false),
                    running: None,
                };
                resp.print_and_exit();
            }
            err(format!("open_service failed: {e}")).print_and_exit();
        }
    };

    // Best-effort stop if running. Поллим до 5 сек.
    if let Ok(status) = svc.query_status() {
        if status.current_state != ServiceState::Stopped {
            let _ = svc.stop();
            for _ in 0..50 {
                std::thread::sleep(Duration::from_millis(100));
                if let Ok(s) = svc.query_status() {
                    if s.current_state == ServiceState::Stopped {
                        break;
                    }
                }
            }
        }
    }

    if let Err(e) = svc.delete() {
        err(format!("delete failed: {e}")).print_and_exit();
    }

    let resp = CliResponse {
        ok: true,
        error: None,
        needs_elevation: None,
        service_name: Some(SERVICE_NAME.into()),
        installed: Some(false),
        running: Some(false),
    };
    resp.print_and_exit();
}

pub fn start() -> ! {
    let scm = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        Ok(s) => s,
        Err(e) => map_open_scm_err(e).print_and_exit(),
    };
    let svc = match scm.open_service(SERVICE_NAME, ServiceAccess::START | ServiceAccess::QUERY_STATUS)
    {
        Ok(s) => s,
        Err(e) => err(format!("open_service failed: {e}")).print_and_exit(),
    };
    if let Err(e) = svc.start::<&str>(&[]) {
        // ERROR_SERVICE_ALREADY_RUNNING (1056) — idempotent ok.
        let msg = format!("{e}");
        if !msg.contains("1056") && !msg.to_lowercase().contains("already running") {
            err(format!("start failed: {e}")).print_and_exit();
        }
    }
    let resp = CliResponse {
        ok: true,
        error: None,
        needs_elevation: None,
        service_name: Some(SERVICE_NAME.into()),
        installed: Some(true),
        running: Some(true),
    };
    resp.print_and_exit();
}

pub fn stop() -> ! {
    let scm = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        Ok(s) => s,
        Err(e) => map_open_scm_err(e).print_and_exit(),
    };
    let svc = match scm.open_service(SERVICE_NAME, ServiceAccess::STOP | ServiceAccess::QUERY_STATUS)
    {
        Ok(s) => s,
        Err(e) => err(format!("open_service failed: {e}")).print_and_exit(),
    };
    let _ = svc.stop();
    let resp = CliResponse {
        ok: true,
        error: None,
        needs_elevation: None,
        service_name: Some(SERVICE_NAME.into()),
        installed: Some(true),
        running: Some(false),
    };
    resp.print_and_exit();
}

pub fn status() -> ! {
    let scm = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        Ok(s) => s,
        Err(e) => {
            // open SCM с CONNECT обычно работает user-mode; если упало — true error.
            err(format!("open SCM failed: {e}")).print_and_exit();
        }
    };
    match scm.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS) {
        Ok(svc) => {
            let running = svc
                .query_status()
                .map(|s| s.current_state == ServiceState::Running)
                .unwrap_or(false);
            let resp = CliResponse {
                ok: true,
                error: None,
                needs_elevation: None,
                service_name: Some(SERVICE_NAME.into()),
                installed: Some(true),
                running: Some(running),
            };
            resp.print_and_exit();
        }
        Err(_) => {
            let resp = CliResponse {
                ok: true,
                error: None,
                needs_elevation: None,
                service_name: Some(SERVICE_NAME.into()),
                installed: Some(false),
                running: Some(false),
            };
            resp.print_and_exit();
        }
    }
}
