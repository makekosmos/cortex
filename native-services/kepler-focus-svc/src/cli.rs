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
use kepler_focus_svc::{uninstall_service_names, LEGACY_SERVICE_NAME, SERVICE_NAME};

pub const SERVICE_DISPLAY_NAME: &str = "Kosmos System Service";
pub const SERVICE_DESCRIPTION: &str =
    "Privileged local service для Kosmos: hosts blocking и fast NTFS file indexing. \
Можно безопасно удалить: sc stop KosmosSystemSvc && sc delete KosmosSystemSvc.";

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
    if lower.contains("access is denied")
        || lower.contains("access denied")
        || lower.contains("os error 5")
    {
        err_elevation(msg)
    } else {
        err(msg)
    }
}

fn exe_path() -> Result<std::path::PathBuf, CliResponse> {
    std::env::current_exe().map_err(|e| err(format!("current_exe failed: {e}")))
}

fn open_installed_service(
    scm: &ServiceManager,
    access: ServiceAccess,
) -> Result<(windows_service::service::Service, &'static str), windows_service::Error> {
    match scm.open_service(SERVICE_NAME, access) {
        Ok(svc) => Ok((svc, SERVICE_NAME)),
        Err(primary) => match scm.open_service(LEGACY_SERVICE_NAME, access) {
            Ok(svc) => Ok((svc, LEGACY_SERVICE_NAME)),
            Err(_) => Err(primary),
        },
    }
}

fn is_missing_service_error(e: &windows_service::Error) -> bool {
    let msg = format!("{e}").to_lowercase();
    msg.contains("does not exist") || msg.contains("1060")
}

fn stop_and_delete_service(
    svc: windows_service::service::Service,
    service_name: &str,
) -> Result<(), String> {
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

    svc.delete()
        .map_err(|e| format!("delete {service_name} failed: {e}"))
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
        // AutoStart — service поднимается на каждом boot'е без admin.
        // Гарантирует zero-UAC focus mode после первой установки: pipe всегда
        // доступен, Kepler не нуждается в правах для запуска service'а.
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: vec![OsString::from("run-as-service")],
        dependencies: vec![],
        // LocalSystem = None.
        account_name: None,
        account_password: None,
    };

    let svc = match scm.create_service(
        &info,
        ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::QUERY_STATUS,
    ) {
        Ok(s) => s,
        Err(e) => err(format!("create_service failed: {e}")).print_and_exit(),
    };

    // Description — best-effort, ignore failure.
    let _ = svc.set_description(SERVICE_DESCRIPTION);

    // Запускаем сразу — мы уже elevated, бесплатно. Юзер сразу получает
    // working pipe без необходимости вызывать start (который без admin
    // не сработает на свежеустановленном service'е).
    let mut running = false;
    if let Err(e) = svc.start::<&str>(&[]) {
        let msg = format!("{e}");
        if msg.contains("1056") || msg.to_lowercase().contains("already running") {
            running = true;
        }
        // Иначе install OK, но start не получился — вернём ok с running=false,
        // shell сам разберётся (пользователь увидит fallback).
    } else {
        // Подождём пока state перейдёт в Running (poll up to 3s).
        for _ in 0..30 {
            if let Ok(s) = svc.query_status() {
                if s.current_state == ServiceState::Running {
                    running = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

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

pub fn uninstall() -> ! {
    let scm = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        Ok(s) => s,
        Err(e) => map_open_scm_err(e).print_and_exit(),
    };

    let mut deleted_any = false;
    for service_name in uninstall_service_names() {
        // См. postmortems.md § 2026-05-26: upgrade может оставить оба сервиса.
        let svc = match scm.open_service(
            service_name,
            ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
        ) {
            Ok(s) => s,
            Err(e) if is_missing_service_error(&e) => continue,
            Err(e) => err(format!("open_service {service_name} failed: {e}")).print_and_exit(),
        };
        if let Err(e) = stop_and_delete_service(svc, service_name) {
            err(e).print_and_exit();
        }
        deleted_any = true;
    }

    let resp = CliResponse {
        ok: true,
        error: None,
        needs_elevation: None,
        service_name: Some(SERVICE_NAME.into()),
        installed: Some(false),
        running: Some(false).filter(|_| deleted_any),
    };
    resp.print_and_exit();
}

pub fn start() -> ! {
    let scm = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        Ok(s) => s,
        Err(e) => map_open_scm_err(e).print_and_exit(),
    };
    let (svc, service_name) =
        match open_installed_service(&scm, ServiceAccess::START | ServiceAccess::QUERY_STATUS) {
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
        service_name: Some(service_name.into()),
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
    let (svc, service_name) =
        match open_installed_service(&scm, ServiceAccess::STOP | ServiceAccess::QUERY_STATUS) {
            Ok(s) => s,
            Err(e) => err(format!("open_service failed: {e}")).print_and_exit(),
        };
    let _ = svc.stop();
    let resp = CliResponse {
        ok: true,
        error: None,
        needs_elevation: None,
        service_name: Some(service_name.into()),
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
    match open_installed_service(&scm, ServiceAccess::QUERY_STATUS) {
        Ok((svc, service_name)) => {
            let running = svc
                .query_status()
                .map(|s| s.current_state == ServiceState::Running)
                .unwrap_or(false);
            let resp = CliResponse {
                ok: true,
                error: None,
                needs_elevation: None,
                service_name: Some(service_name.into()),
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
