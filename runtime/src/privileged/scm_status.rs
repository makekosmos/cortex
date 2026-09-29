//! User-mode SCM queries — status reads, service PID lookup, `privileged
//! status` CLI output. None of this elevates.

#![cfg(windows)]

use windows_service::service::{ServiceAccess, ServiceState};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

use crate::privileged::brand;
use crate::privileged::cli::Outcome;
use crate::privileged::scm::err_outcome;

#[derive(Debug, Clone, Default)]
pub struct ServiceStatus {
    pub installed: bool,
    pub running: bool,
    pub binary_path: Option<String>,
}

fn is_missing_service_error(e: &windows_service::Error) -> bool {
    let msg = format!("{e}").to_lowercase();
    msg.contains("does not exist") || msg.contains("1060")
}

/// User-mode status query — never elevates.
pub fn query_status() -> Result<ServiceStatus, String> {
    let scm = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| format!("open SCM failed: {e}"))?;
    match scm.open_service(
        brand::SERVICE_NAME,
        ServiceAccess::QUERY_STATUS | ServiceAccess::QUERY_CONFIG,
    ) {
        Ok(svc) => {
            let running = svc
                .query_status()
                .map(|s| s.current_state == ServiceState::Running)
                .unwrap_or(false);
            let binary_path = svc
                .query_config()
                .ok()
                .map(|c| c.executable_path.to_string_lossy().into_owned());
            Ok(ServiceStatus {
                installed: true,
                running,
                binary_path,
            })
        }
        Err(e) if is_missing_service_error(&e) => Ok(ServiceStatus::default()),
        Err(e) => Err(format!("open_service failed: {e}")),
    }
}

/// PID of a running service's process — used by the pipe client to verify
/// the named-pipe server identity (anti-squatting). `None` when the service
/// is missing or stopped.
pub fn service_pid(service_name: &str) -> Option<u32> {
    let scm = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).ok()?;
    let svc = scm
        .open_service(service_name, ServiceAccess::QUERY_STATUS)
        .ok()?;
    svc.query_status().ok()?.process_id
}

/// CLI `privileged status` — JSON, user-mode.
pub fn status_cli() -> Outcome {
    match query_status() {
        Ok(status) => Outcome {
            ok: true,
            service_name: Some(brand::SERVICE_NAME.into()),
            installed: Some(status.installed),
            running: Some(status.running),
            service_dir: status.binary_path,
            ..Outcome::default()
        },
        Err(e) => err_outcome(e),
    }
}
