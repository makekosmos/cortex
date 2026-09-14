use std::path::Path;
#[cfg(windows)]
pub(crate) use std::path::PathBuf;
pub(crate) use std::process::{ExitCode, Stdio};
pub(crate) use std::time::{Duration, Instant};

pub(crate) use chrono::Utc;
pub(crate) use serde::Serialize;
pub(crate) use tokio::process::{Child, Command};

pub(crate) use crate::engine_control::{self, ControlMessage, ControlServer, ControlState};
pub(crate) use crate::lock_file;
pub(crate) use crate::singleton::{SingletonError, SingletonGuard};

mod backoff;
mod control;
mod lease;
mod mode;
mod model;
mod monitor;
mod spawn;
mod state;
mod supervisor;

pub(crate) use backoff::*;
pub(crate) use control::*;
pub(crate) use lease::*;
pub(crate) use model::*;
use monitor::ChildResult;
pub(crate) use monitor::*;
pub(crate) use spawn::*;
pub(crate) use state::*;

#[cfg(test)]
mod tests_control;
#[cfg(test)]
mod tests_lifecycle;
#[cfg(test)]
mod tests_modes;

pub use mode::process_mode;
pub use model::{
    ProcessMode, CORE_WORKER_ARG, RESTART_CORE_ARG, SHUTDOWN_ARG, START_ARG, TRAY_EXIT_CODE,
};

pub async fn run_supervisor() -> ExitCode {
    supervisor::run_supervisor().await
}

pub fn restart_core() -> ExitCode {
    control::restart_core()
}

pub fn shutdown() -> ExitCode {
    control::shutdown()
}

pub fn diagnostics_snapshot(data_dir: &Path) -> serde_json::Value {
    state::diagnostics_snapshot(data_dir)
}
