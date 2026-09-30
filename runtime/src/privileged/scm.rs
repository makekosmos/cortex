//! Service Control Manager operations for the privileged service.
//!
//! `install`/`uninstall` require elevation (invoked via the `runas` verb from
//! `system.privileged.enable`); `query_status` works from user mode.
//!
//! The service binary is a copy of the Engine exe placed under
//! `%ProgramFiles%\<Brand>\Service\` — an admin-only-writable location.
//! A service binary under `%LOCALAPPDATA%`/other user-writable roots would be
//! a privilege-escalation vector (any user-mode process could swap the binary
//! SYSTEM runs), so the path policy below rejects them outright.

#![cfg(windows)]

use std::ffi::OsString;
use std::time::Duration;

use windows_service::service::{
    Service, ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceState,
    ServiceType,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

use crate::privileged::brand;
use crate::privileged::cli::Outcome;
use crate::privileged::install_path::{env_get, install_dir};
use crate::privileged::token::validate_grant_sid;

pub(crate) fn err_outcome(msg: impl Into<String>) -> Outcome {
    Outcome {
        ok: false,
        error: Some(msg.into()),
        ..Outcome::default()
    }
}

fn is_access_denied(e: &windows_service::Error) -> bool {
    let msg = format!("{e}").to_lowercase();
    msg.contains("access is denied") || msg.contains("access denied") || msg.contains("os error 5")
}

fn is_missing_service_error(e: &windows_service::Error) -> bool {
    let msg = format!("{e}").to_lowercase();
    msg.contains("does not exist") || msg.contains("1060")
}

fn open_scm(create: bool) -> Result<ServiceManager, Outcome> {
    let access = if create {
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE
    } else {
        ServiceManagerAccess::CONNECT
    };
    ServiceManager::local_computer(None::<&str>, access).map_err(|e| {
        let msg = format!("open SCM failed: {e}");
        if is_access_denied(&e) {
            Outcome {
                needs_elevation: Some(true),
                ..err_outcome(msg)
            }
        } else {
            err_outcome(msg)
        }
    })
}

fn stop_and_delete_service(svc: &Service, service_name: &str) -> Result<(), String> {
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

/// Elevated install: copy the Engine exe to `%ProgramFiles%\<Brand>\Service`,
/// remove legacy services, register + start `SERVICE_NAME` as LocalSystem.
///
/// `grant_sid` is the SID of the *unelevated* user that ran the Engine — the
/// only account allowed on the pipe. It must come from the client via
/// `--grant-sid`; under over-the-shoulder UAC the elevated process token is
/// the *admin's*, so deriving it locally here would lock out the real user.
pub fn install(grant_sid: &str) -> Outcome {
    // Validate before any SCM/filesystem work: missing or malformed is an
    // error, never a silent fallback to the (admin) process SID.
    if !validate_grant_sid(grant_sid) {
        return err_outcome(format!(
            "invalid or missing --grant-sid ({grant_sid:?}) — refusing to install"
        ));
    }

    let scm = match open_scm(true) {
        Ok(s) => s,
        Err(o) => return o,
    };

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return err_outcome(format!("current_exe failed: {e}")),
    };
    let dir = match install_dir(&env_get) {
        Ok(d) => d,
        Err(e) => return err_outcome(e),
    };
    let service_exe = dir.join(brand::SERVICE_BINARY_NAME);

    // MIGRATION(KOS-267): remove after 2026-11-01 — drop pre-Engine services.
    for legacy in brand::LEGACY_SERVICE_NAMES {
        if let Ok(svc) = scm.open_service(
            legacy,
            ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
        ) {
            let _ = stop_and_delete_service(&svc, legacy);
        }
    }

    // Reinstall path: a running service locks its binary — stop and delete it
    // before overwriting the copy.
    if let Ok(existing) = scm.open_service(
        brand::SERVICE_NAME,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
    ) {
        if let Err(e) = stop_and_delete_service(&existing, brand::SERVICE_NAME) {
            return err_outcome(e);
        }
    }

    if let Err(e) = std::fs::create_dir_all(&dir) {
        return err_outcome(format!("create {} failed: {e}", dir.display()));
    }
    if let Err(e) = std::fs::copy(&exe, &service_exe) {
        return err_outcome(format!(
            "copy {} → {} failed: {e}",
            exe.display(),
            service_exe.display()
        ));
    }

    let info = ServiceInfo {
        name: OsString::from(brand::SERVICE_NAME),
        display_name: OsString::from(brand::SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        // AutoStart: the pipe is available on every boot with no further
        // elevation — one UAC grant covers the product's lifetime.
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_exe.clone(),
        launch_arguments: vec![
            OsString::from("privileged"),
            OsString::from("run-service"),
            OsString::from("--grant-sid"),
            OsString::from(grant_sid),
        ],
        dependencies: vec![],
        // LocalSystem: MFT raw-volume reads and hosts writes both require
        // admin-level access; no lesser account can perform them.
        account_name: None,
        account_password: None,
    };

    let svc = match scm.create_service(
        &info,
        ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::QUERY_STATUS,
    ) {
        Ok(s) => s,
        Err(e) => return err_outcome(format!("create_service failed: {e}")),
    };
    let _ = svc.set_description(brand::SERVICE_DESCRIPTION);

    let mut running = false;
    match svc.start::<&str>(&[]) {
        Ok(()) => {
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
        Err(e) => {
            let msg = format!("{e}");
            // ERROR_SERVICE_ALREADY_RUNNING (1056) — still counts as running.
            if msg.contains("1056") || msg.to_lowercase().contains("already running") {
                running = true;
            }
        }
    }

    Outcome {
        ok: true,
        service_name: Some(brand::SERVICE_NAME.into()),
        installed: Some(true),
        running: Some(running),
        service_dir: Some(dir.to_string_lossy().into_owned()),
        ..Outcome::default()
    }
}

/// Elevated uninstall: stop + delete the service (and legacy leftovers), then
/// remove the `%ProgramFiles%` binary copy and directory.
pub fn uninstall() -> Outcome {
    let scm = match open_scm(false) {
        Ok(s) => s,
        Err(o) => return o,
    };

    let mut deleted_any = false;
    let mut names: Vec<&str> = vec![brand::SERVICE_NAME];
    // MIGRATION(KOS-267): remove after 2026-11-01.
    names.extend(brand::LEGACY_SERVICE_NAMES);
    for name in names {
        match scm.open_service(
            name,
            ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
        ) {
            Ok(svc) => {
                if let Err(e) = stop_and_delete_service(&svc, name) {
                    return err_outcome(e);
                }
                deleted_any = true;
            }
            Err(e) if is_missing_service_error(&e) => continue,
            Err(e) => return err_outcome(format!("open_service {name} failed: {e}")),
        }
    }

    let dir = install_dir(&env_get).unwrap_or_default();
    if !dir.as_os_str().is_empty() {
        let binary = dir.join(brand::SERVICE_BINARY_NAME);
        // `privileged uninstall` runs *from* this binary — a running exe
        // can't be unlinked, so fall back to delete-on-next-boot.
        if std::fs::remove_file(&binary).is_err() {
            crate::privileged::install_path::delete_on_reboot(&binary);
        }
        // Only removed when empty — we never rmdir a dir we don't own blindly.
        let _ = std::fs::remove_dir(&dir);
        let _ = std::fs::remove_dir(dir.parent().unwrap_or(&dir));
    }

    Outcome {
        ok: true,
        service_name: Some(brand::SERVICE_NAME.into()),
        installed: Some(false),
        running: deleted_any.then_some(false),
        service_dir: Some(dir.to_string_lossy().into_owned()),
        ..Outcome::default()
    }
}
