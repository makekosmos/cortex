use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
use std::process::{ExitCode, Stdio};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;
use tokio::process::{Child, Command};

use crate::engine_control::{self, ControlMessage, ControlServer, ControlState};
use crate::lock_file;
use crate::singleton::{SingletonError, SingletonGuard};

pub const CORE_WORKER_ARG: &str = "--core-worker";
pub const RESTART_CORE_ARG: &str = "--restart-core";
pub const SHUTDOWN_ARG: &str = "--shutdown";
pub const START_ARG: &str = "--start";

const RESTART_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(5),
    Duration::from_secs(30),
];
const SUCCESSFUL_RUN: Duration = Duration::from_secs(5 * 60);
const GRACEFUL_STOP_DEADLINE: Duration = Duration::from_secs(5);
const SUPERVISOR_LOCK_FILE: &str = "engine-supervisor.lock.db";
const SUPERVISOR_STATE_FILE: &str = "engine-supervisor-state.json";
const CONTROL_STATE_FILE: &str = "engine-supervisor-control.json";
const SYNC_ENV_KEYS: &[&str] = &[
    "KOSMOS_IROH",
    "KOSMOS_SPACE_ID",
    "KOSMOS_AUTH_SECRET",
    "KOSMOS_IROH_PEER_TICKET",
    "KOSMOS_DEVICE_ID",
    "KOSMOS_DEVICE_NAME",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessMode {
    Supervisor,
    CoreWorker,
    RestartCore,
    Shutdown,
}

#[derive(Debug, Serialize)]
struct SupervisorState<'a> {
    format_version: u32,
    component: &'static str,
    version: &'static str,
    correlation_id: String,
    supervisor_pid: u32,
    core_pid: Option<u32>,
    status: &'a str,
    crash_streak: usize,
    last_exit_code: Option<i32>,
    last_run_ms: Option<u64>,
    updated_at: String,
}

struct ControlStateCleanup<'a>(&'a Path);

impl Drop for ControlStateCleanup<'_> {
    fn drop(&mut self) {
        cleanup_state(self.0);
    }
}

#[derive(Debug, Clone, Copy)]
struct ExitMetadata {
    code: Option<i32>,
    run_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildAttemptAction {
    RotateCredentials,
    BindEndpoint,
    PublishEndpoint,
    SpawnChild,
}

#[derive(Debug, PartialEq, Eq)]
struct ChildAttempt {
    generation: u64,
    actions: [ChildAttemptAction; 4],
}

struct ChildLifecycle {
    generation: u64,
}

impl ChildLifecycle {
    fn new(generation: u64) -> Self {
        Self { generation }
    }

    fn next_attempt(&mut self) -> ChildAttempt {
        self.generation = self.generation.saturating_add(1);
        ChildAttempt {
            generation: self.generation,
            actions: [
                ChildAttemptAction::RotateCredentials,
                ChildAttemptAction::BindEndpoint,
                ChildAttemptAction::PublishEndpoint,
                ChildAttemptAction::SpawnChild,
            ],
        }
    }
}

enum SpawnAttemptError {
    Control,
    State,
    Spawn(std::io::Error),
}

fn take_desktop_lease_env() -> Result<Option<(u32, String)>, ()> {
    let credential = std::env::var("KOSMOS_DESKTOP_ROLE_CREDENTIAL").ok();
    let pid = std::env::var("KOSMOS_DESKTOP_ROLE_PID").ok();
    std::env::remove_var("KOSMOS_DESKTOP_ROLE_CREDENTIAL");
    std::env::remove_var("KOSMOS_DESKTOP_ROLE_PID");
    match (credential, pid) {
        (None, None) => Ok(None),
        (Some(credential), Some(pid)) => {
            let pid = pid.parse::<u32>().map_err(|_| ())?;
            if pid == 0 || credential.is_empty() || credential.len() > 128 {
                return Err(());
            }
            Ok(Some((pid, credential)))
        }
        _ => Err(()),
    }
}
pub fn process_mode(args: impl IntoIterator<Item = String>) -> ProcessMode {
    let mut mode = ProcessMode::Supervisor;
    for arg in args {
        match arg.as_str() {
            CORE_WORKER_ARG => return ProcessMode::CoreWorker,
            RESTART_CORE_ARG => return ProcessMode::RestartCore,
            SHUTDOWN_ARG => return ProcessMode::Shutdown,
            START_ARG => mode = ProcessMode::Supervisor,
            _ => {}
        }
    }
    mode
}

pub async fn run_supervisor() -> ExitCode {
    let mut pending_desktop_lease = match take_desktop_lease_env() {
        Ok(value) => value,
        Err(()) => return ExitCode::from(2),
    };
    let data_dir = match lock_file::kosmos_data_dir() {
        Ok(path) => path,
        Err(error) => {
            crate::observability::stderr(format!(
                "[kosmos-engine] failed to resolve data dir: {error}"
            ));
            return ExitCode::from(1);
        }
    };
    let correlation_id = lock_file::read_engine(&data_dir.join(lock_file::ENGINE_LOCK_FILE_NAME))
        .ok()
        .filter(|lock| process_matches_current_executable(lock.pid))
        .map(|lock| lock.correlation_id)
        .filter(|value| uuid::Uuid::parse_str(value).is_ok())
        .unwrap_or_else(crate::observability::correlation_id);
    std::env::set_var(crate::observability::CORRELATION_ID_ENV, &correlation_id);
    let supervisor_lock = data_dir.join(SUPERVISOR_LOCK_FILE);
    let _guard = match SingletonGuard::acquire(&supervisor_lock) {
        Ok(guard) => guard,
        Err(SingletonError::AlreadyRunning) => {
            // `start` is an idempotent ensure operation. Electron and native
            // launchers may both call it without creating duplicate cores.
            let Some((electron_pid, credential)) = pending_desktop_lease else {
                return ExitCode::SUCCESS;
            };
            let state = match lock_file::read_owner_only_json::<ControlState>(
                &data_dir.join(CONTROL_STATE_FILE),
            ) {
                Ok(state) => state,
                Err(_) => return ExitCode::from(2),
            };
            return match engine_control::send_control(
                &state,
                ControlMessage::DesktopLease {
                    electron_pid,
                    credential,
                },
            )
            .await
            {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(2),
            };
        }
        Err(error) => {
            crate::observability::stderr(format!(
                "[kosmos-engine] supervisor singleton failed: {error}"
            ));
            return ExitCode::from(1);
        }
    };

    let state_path = data_dir.join(SUPERVISOR_STATE_FILE);
    let control_state_path = data_dir.join(CONTROL_STATE_FILE);
    let _control_cleanup = ControlStateCleanup(&control_state_path);
    let engine_lock_path = data_dir.join(lock_file::ENGINE_LOCK_FILE_NAME);
    if running_engine_pid(&engine_lock_path).is_some() {
        crate::observability::stderr(
            "[kosmos-engine] supervisor unavailable: existing core has no verifiable control ownership",
        );
        return ExitCode::from(2);
    }
    let session_id = uuid::Uuid::new_v4().to_string();
    let controller_secret = engine_control::new_secret();
    // The controller secret is persisted only in the owner-readable control state.
    // The core secret is injected into the owned child environment and is never
    // written to controller state, logs, renderer data, or evidence artifacts.
    let mut core_secret = engine_control::new_secret();
    let control_state = match ControlServer::bind(
        session_id,
        1,
        format!("uid:{}", current_owner_identity()),
        controller_secret.clone(),
        core_secret.clone(),
    )
    .await
    {
        Ok((server, endpoint)) => {
            let state = ControlState {
                endpoint,
                supervisor_session_id: server.session_id().to_string(),
                child_generation: 1,
                owner_identity: server.owner_identity().to_string(),
                secret: engine_control::encode_secret(&controller_secret),
            };
            if lock_file::write_owner_only_json(&control_state_path, &state).is_err() {
                return ExitCode::from(1);
            }
            (state, server)
        }
        Err(_) => return ExitCode::from(1),
    };
    let (mut control_state, mut control_server) = control_state;
    let mut crash_streak = 0usize;
    let mut last_exit = None;
    let mut first_spawn = true;
    let mut lifecycle = ChildLifecycle::new(control_state.child_generation);

    loop {
        if let Some(pid) = running_engine_pid(&engine_lock_path) {
            write_state(
                &state_path,
                "running",
                crash_streak,
                Some(pid),
                last_exit.as_ref(),
            );
            match monitor_existing_core(&engine_lock_path).await {
                MonitorResult::Shutdown => {
                    crate::observability::stderr(
                        "[kosmos-engine] cannot stop an existing core without authenticated ownership",
                    );
                    cleanup_state(&state_path);
                    cleanup_state(&control_state_path);
                    return ExitCode::from(1);
                }
                MonitorResult::Exited { ran_for } => {
                    last_exit = Some(ExitMetadata {
                        code: None,
                        run_ms: duration_ms(ran_for),
                    });
                    crash_streak =
                        crash_streak_after_exit(crash_streak, ran_for, ExitKind::Unexpected);
                }
            }
        } else {
            let replacement = !first_spawn;
            first_spawn = false;
            let mut child = match spawn_child_attempt(
                &data_dir,
                &control_state_path,
                &mut control_state,
                &mut control_server,
                &mut core_secret,
                &mut lifecycle,
                replacement,
            )
            .await
            {
                Ok(child) => child,
                Err(error) => {
                    if matches!(
                        &error,
                        SpawnAttemptError::Control | SpawnAttemptError::State
                    ) {
                        return ExitCode::from(1);
                    }
                    crate::observability::stderr(format!(
                        "[kosmos-engine] failed to spawn core worker: {}",
                        match error {
                            SpawnAttemptError::Spawn(error) => error,
                            SpawnAttemptError::Control | SpawnAttemptError::State => unreachable!(),
                        }
                    ));
                    crash_streak = crash_streak.saturating_add(1);
                    if crash_streak > RESTART_DELAYS.len() {
                        write_state(
                            &state_path,
                            "failed",
                            crash_streak,
                            None,
                            last_exit.as_ref(),
                        );
                        return ExitCode::from(1);
                    }
                    if !wait_before_restart(&state_path, crash_streak, None, last_exit.as_ref())
                        .await
                    {
                        cleanup_state(&state_path);
                        cleanup_state(&control_state_path);
                        return ExitCode::SUCCESS;
                    }
                    continue;
                }
            };
            let pid = child.id();
            if let Some((electron_pid, credential)) = pending_desktop_lease.take() {
                if send_desktop_lease_when_ready(&mut control_server, electron_pid, credential)
                    .await
                    .is_err()
                {
                    let _ = child.kill().await;
                    return ExitCode::from(1);
                }
            }
            let started_at = Instant::now();
            write_state(
                &state_path,
                "running",
                crash_streak,
                pid,
                last_exit.as_ref(),
            );
            match wait_for_child(&mut child, &mut control_server).await {
                ChildResult::Control(ControlMessage::RestartRequested) => {
                    stop_owned_child_gracefully(&mut child, &mut control_server).await;
                    let _ = std::fs::remove_file(&engine_lock_path);
                    write_state(
                        &state_path,
                        "restarting",
                        crash_streak,
                        None,
                        last_exit.as_ref(),
                    );
                    continue;
                }
                ChildResult::Control(ControlMessage::ShutdownRequested) => {
                    stop_owned_child_gracefully(&mut child, &mut control_server).await;
                    cleanup_state(&state_path);
                    cleanup_state(&control_state_path);
                    return ExitCode::SUCCESS;
                }
                ChildResult::Control(_) => continue,
                ChildResult::Shutdown => {
                    cleanup_state(&state_path);
                    cleanup_state(&control_state_path);
                    return ExitCode::SUCCESS;
                }
                ChildResult::Exited(status) => {
                    let ran_for = started_at.elapsed();
                    last_exit = Some(ExitMetadata {
                        code: status.code(),
                        run_ms: duration_ms(ran_for),
                    });
                    crate::observability::stderr(format!(
                        "[kosmos-engine] core exited with {status} after {:.1}s",
                        ran_for.as_secs_f64()
                    ));
                    crash_streak =
                        crash_streak_after_exit(crash_streak, ran_for, ExitKind::Unexpected);
                }
            }
        }

        if crash_streak > RESTART_DELAYS.len() {
            write_state(
                &state_path,
                "failed",
                crash_streak,
                None,
                last_exit.as_ref(),
            );
            crate::observability::stderr(format!(
                "[kosmos-engine] core failed {} times; automatic restart stopped",
                RESTART_DELAYS.len()
            ));
            return ExitCode::from(1);
        }

        if !wait_before_restart(&state_path, crash_streak, None, last_exit.as_ref()).await {
            cleanup_state(&state_path);
            cleanup_state(&control_state_path);
            return ExitCode::SUCCESS;
        }
    }
}

pub fn restart_core() -> ExitCode {
    let data_dir = match lock_file::kosmos_data_dir() {
        Ok(path) => path,
        Err(error) => {
            crate::observability::stderr(format!(
                "[kosmos-engine] restart failed to resolve data dir: {error}"
            ));
            return ExitCode::from(1);
        }
    };
    match read_control_state(&data_dir).and_then(|state| {
        engine_control::send_control_blocking(&state, ControlMessage::RestartRequested)
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            crate::observability::stderr(format!("[kosmos-engine] restart refused: {error}"));
            exit_code_for_control_error(error)
        }
    }
}

pub fn shutdown() -> ExitCode {
    let data_dir = match lock_file::kosmos_data_dir() {
        Ok(path) => path,
        Err(error) => {
            crate::observability::stderr(format!(
                "[kosmos-engine] shutdown failed to resolve data dir: {error}"
            ));
            return ExitCode::from(1);
        }
    };
    match read_control_state(&data_dir).and_then(|state| {
        engine_control::send_control_blocking(&state, ControlMessage::ShutdownRequested)
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            crate::observability::stderr(format!("[kosmos-engine] shutdown refused: {error}"));
            exit_code_for_control_error(error)
        }
    }
}

async fn send_desktop_lease_when_ready(
    control_server: &mut ControlServer,
    electron_pid: u32,
    credential: String,
) -> Result<(), engine_control::ControlError> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match control_server
            .send_to_core(ControlMessage::DesktopLease {
                electron_pid,
                credential: credential.clone(),
            })
            .await
        {
            Ok(()) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                match tokio::time::timeout(remaining, async {
                    loop {
                        match control_server.recv().await {
                            Some(ControlMessage::DesktopLeaseInstalled {
                                generation,
                                electron_pid: ack_pid,
                            }) if generation == control_server.child_generation()
                                && ack_pid == electron_pid =>
                            {
                                return Ok(())
                            }
                            Some(_) => continue,
                            None => return Err(engine_control::ControlError::Unavailable),
                        }
                    }
                })
                .await
                {
                    Ok(result) => {
                        if let Err(ref error) = result {
                            crate::observability::stderr(format!(
                                "[kosmos-engine] desktop lease acknowledgement failed: {error}"
                            ));
                        }
                        return result;
                    }
                    Err(_) => return Err(engine_control::ControlError::Timeout),
                }
            }
            Err(error) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(10)).await;
                let _ = error;
            }
            Err(error) => return Err(error),
        }
    }
}

fn read_control_state(data_dir: &Path) -> Result<ControlState, engine_control::ControlError> {
    let path = data_dir.join(CONTROL_STATE_FILE);
    let bytes = std::fs::read(path).map_err(|_| engine_control::ControlError::Unavailable)?;
    serde_json::from_slice(&bytes).map_err(|_| engine_control::ControlError::Unavailable)
}

fn exit_code_for_control_error(error: engine_control::ControlError) -> ExitCode {
    match error {
        engine_control::ControlError::Unavailable => ExitCode::from(2),
        _ => ExitCode::from(1),
    }
}

fn current_owner_identity() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

fn spawn_core_worker(
    data_dir: &Path,
    control_state: &ControlState,
    core_secret: &[u8],
) -> Result<Child, std::io::Error> {
    let executable = std::env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg(CORE_WORKER_ARG)
        .env("KOSMOS_ENGINE_SUPERVISED", "1")
        .env("KOSMOS_CONTROL_ENDPOINT", &control_state.endpoint)
        .env(
            "KOSMOS_CONTROL_SESSION_ID",
            &control_state.supervisor_session_id,
        )
        .env(
            "KOSMOS_CONTROL_GENERATION",
            control_state.child_generation.to_string(),
        )
        .env("KOSMOS_CONTROL_OWNER_ID", &control_state.owner_identity)
        .env(
            "KOSMOS_CORE_CONTROL_SECRET",
            engine_control::encode_secret(core_secret),
        )
        .envs(read_sync_env(data_dir))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    // См. postmortems.md § 2026-07-31: supervised workers must stay background-only.
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command.spawn()
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

async fn spawn_child_attempt(
    data_dir: &Path,
    control_state_path: &Path,
    control_state: &mut ControlState,
    control_server: &mut ControlServer,
    core_secret: &mut Vec<u8>,
    lifecycle: &mut ChildLifecycle,
    replacement: bool,
) -> Result<Child, SpawnAttemptError> {
    if replacement {
        let attempt = lifecycle.next_attempt();
        control_server.abort();
        *core_secret = engine_control::new_secret();
        let controller_secret = engine_control::decode_secret(&control_state.secret)
            .map_err(|_| SpawnAttemptError::Control)?;
        let (server, endpoint) = ControlServer::bind(
            control_state.supervisor_session_id.clone(),
            attempt.generation,
            control_state.owner_identity.clone(),
            controller_secret,
            core_secret.clone(),
        )
        .await
        .map_err(|_| SpawnAttemptError::Control)?;
        control_state.child_generation = attempt.generation;
        control_state.endpoint = endpoint;
        *control_server = server;
        lock_file::write_owner_only_json(control_state_path, control_state)
            .map_err(|_| SpawnAttemptError::State)?;
    }
    spawn_core_worker(data_dir, control_state, core_secret).map_err(SpawnAttemptError::Spawn)
}

fn read_sync_env(data_dir: &Path) -> Vec<(String, String)> {
    let Ok(contents) = std::fs::read_to_string(data_dir.join("sync.env")) else {
        return Vec::new();
    };
    contents
        .lines()
        .filter_map(|raw| {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            SYNC_ENV_KEYS
                .contains(&key)
                .then(|| (key.to_string(), value.trim().to_string()))
        })
        .collect()
}

fn running_engine_pid(lock_path: &Path) -> Option<u32> {
    let lock = lock_file::read_engine(lock_path).ok()?;
    process_matches_current_executable(lock.pid).then_some(lock.pid)
}

enum MonitorResult {
    Shutdown,
    Exited { ran_for: Duration },
}

async fn monitor_existing_core(lock_path: &Path) -> MonitorResult {
    let started_at = Instant::now();
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => return MonitorResult::Shutdown,
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                if running_engine_pid(lock_path).is_none() {
                    return MonitorResult::Exited { ran_for: started_at.elapsed() };
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ChildResult {
    Shutdown,
    Control(ControlMessage),
    Exited(std::process::ExitStatus),
}

async fn wait_for_child(child: &mut Child, control: &mut ControlServer) -> ChildResult {
    loop {
        tokio::select! {
            result = child.wait() => {
                return match result {
                    Ok(status) => ChildResult::Exited(status),
                    Err(error) => {
                        crate::observability::stderr(format!(
                            "[kosmos-engine] wait for core failed: {error}"
                        ));
                        ChildResult::Exited(exit_status_failure())
                    }
                };
            }
            message = control.recv() => {
                match message {
                    Some(ControlMessage::RestartRequested) | Some(ControlMessage::ShutdownRequested) => {
                        return ChildResult::Control(message.expect("control message"));
                    }
                    Some(ControlMessage::CoreReady) | Some(ControlMessage::CoreStopping) => {}
                    Some(message @ ControlMessage::DesktopLease { .. }) => {
                        let _ = control.send_to_core(message).await;
                    }
                    Some(ControlMessage::DesktopLeaseRevoked { .. })
                    | Some(ControlMessage::DesktopLeaseInstalled { .. }) => {}
                    None => return ChildResult::Shutdown,
                }
            }
        }
    }
}

#[cfg(test)]
async fn stop_owned_child(child: &mut Child) {
    stop_owned_child_with_deadline(child, GRACEFUL_STOP_DEADLINE).await;
}

async fn stop_owned_child_gracefully(child: &mut Child, control: &mut ControlServer) {
    stop_owned_child_gracefully_with_deadline(child, control, GRACEFUL_STOP_DEADLINE).await;
}

async fn stop_owned_child_gracefully_with_deadline(
    child: &mut Child,
    control: &mut ControlServer,
    graceful_deadline: Duration,
) {
    let deadline = tokio::time::sleep(graceful_deadline);
    tokio::pin!(deadline);
    let mut core_channel_open = true;
    loop {
        tokio::select! {
            result = child.wait() => {
                if result.is_err() {
                    let _ = child.kill().await;
                }
                return;
            }
            message = control.recv(), if core_channel_open => {
                match message {
                    Some(ControlMessage::CoreStopping) => {
                        // CoreStopping is an acknowledgement, not proof of process exit.
                    }
                    None => {
                        // A closed channel is not proof of stopping; keep the same deadline.
                        core_channel_open = false;
                    }
                    Some(_) => {}
                }
            }
            _ = &mut deadline => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return;
            }
        }
    }
}

async fn stop_owned_child_with_deadline(child: &mut Child, deadline: Duration) {
    let exited = tokio::time::timeout(deadline, child.wait()).await;
    if exited.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

async fn wait_before_restart(
    state_path: &Path,
    crash_streak: usize,
    core_pid: Option<u32>,
    last_exit: Option<&ExitMetadata>,
) -> bool {
    let index = crash_streak.saturating_sub(1);
    let Some(delay) = RESTART_DELAYS.get(index).copied() else {
        return true;
    };
    write_state(state_path, "restarting", crash_streak, core_pid, last_exit);
    crate::observability::stderr(format!(
        "[kosmos-engine] restart attempt {}/{} in {}s",
        crash_streak,
        RESTART_DELAYS.len(),
        delay.as_secs()
    ));
    tokio::select! {
        _ = tokio::time::sleep(delay) => true,
        _ = tokio::signal::ctrl_c() => false,
    }
}

fn next_crash_streak(current: usize, ran_for: Duration) -> usize {
    if ran_for >= SUCCESSFUL_RUN {
        1
    } else {
        current.saturating_add(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitKind {
    Intentional,
    Unexpected,
}

fn crash_streak_after_exit(current: usize, ran_for: Duration, kind: ExitKind) -> usize {
    match kind {
        ExitKind::Intentional => current,
        ExitKind::Unexpected => next_crash_streak(current, ran_for),
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

fn write_state(
    path: &Path,
    status: &str,
    crash_streak: usize,
    core_pid: Option<u32>,
    last_exit: Option<&ExitMetadata>,
) {
    let state = SupervisorState {
        format_version: 1,
        component: "engine-core",
        version: env!("CARGO_PKG_VERSION"),
        correlation_id: crate::observability::correlation_id(),
        supervisor_pid: std::process::id(),
        core_pid,
        status,
        crash_streak,
        last_exit_code: last_exit.and_then(|exit| exit.code),
        last_run_ms: last_exit.map(|exit| exit.run_ms),
        updated_at: Utc::now().to_rfc3339(),
    };
    let Ok(json) = serde_json::to_vec_pretty(&state) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let temp = path.with_extension(format!("tmp.{}", std::process::id()));
    if std::fs::write(&temp, json).is_ok() {
        let _ = std::fs::rename(temp, path);
    }
}

pub fn diagnostics_snapshot(data_dir: &Path) -> serde_json::Value {
    std::fs::read(data_dir.join(SUPERVISOR_STATE_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(serde_json::Value::Null)
}

fn cleanup_state(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[cfg(windows)]
fn process_matches_current_executable(pid: u32) -> bool {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let expected = match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(path) => path,
        Err(_) => return false,
    };
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut buffer = vec![0u16; 2048];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        if result.is_err() {
            return false;
        }
        let actual = PathBuf::from(String::from_utf16_lossy(&buffer[..size as usize]));
        std::fs::canonicalize(actual)
            .map(|path| path == expected)
            .unwrap_or(false)
    }
}

#[cfg(unix)]
fn process_matches_current_executable(pid: u32) -> bool {
    let expected = match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(path) => path,
        Err(_) => return false,
    };
    std::fs::canonicalize(format!("/proc/{pid}/exe"))
        .map(|path| path == expected)
        .unwrap_or(false)
}

#[cfg(windows)]
trait WindowsCommandExt {
    fn creation_flags(&mut self, flags: u32) -> &mut Self;
}

#[cfg(windows)]
impl WindowsCommandExt for std::process::Command {
    fn creation_flags(&mut self, flags: u32) -> &mut Self {
        std::os::windows::process::CommandExt::creation_flags(self, flags);
        self
    }
}

#[cfg(unix)]
fn exit_status_failure() -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(1 << 8)
}

#[cfg(windows)]
fn exit_status_failure() -> std::process::ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_control::ControlError;

    #[test]
    fn supervisor_uses_only_engine_lock_contract() {
        let source = include_str!("engine_supervisor.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("supervisor production source");
        assert!(source.contains("ENGINE_LOCK_FILE_NAME"));
        assert!(source.contains("lock_file::read_engine("));
        assert!(!source.contains("lock_file::LOCK_FILE_NAME"));
        assert!(!source.contains("lock_file::read("));
    }

    #[test]
    fn parses_process_modes() {
        assert_eq!(process_mode(Vec::<String>::new()), ProcessMode::Supervisor);
        assert_eq!(
            process_mode([START_ARG.to_string()]),
            ProcessMode::Supervisor
        );
        assert_eq!(
            process_mode([CORE_WORKER_ARG.to_string()]),
            ProcessMode::CoreWorker
        );
        assert_eq!(
            process_mode([RESTART_CORE_ARG.to_string()]),
            ProcessMode::RestartCore
        );
        assert_eq!(
            process_mode([SHUTDOWN_ARG.to_string()]),
            ProcessMode::Shutdown
        );
    }

    #[test]
    fn accepted_restart_mints_one_generation_and_one_child_attempt() {
        let mut lifecycle = ChildLifecycle::new(41);

        let attempt = lifecycle.next_attempt();

        assert_eq!(attempt.generation, 42);
        assert_eq!(
            attempt.actions,
            [
                ChildAttemptAction::RotateCredentials,
                ChildAttemptAction::BindEndpoint,
                ChildAttemptAction::PublishEndpoint,
                ChildAttemptAction::SpawnChild,
            ]
        );
        assert_eq!(lifecycle.generation, 42);
    }

    #[test]
    fn unexpected_exit_and_spawn_retry_each_advance_one_attempt() {
        let mut lifecycle = ChildLifecycle::new(9);

        let unexpected_exit_attempt = lifecycle.next_attempt();
        let spawn_retry_attempt = lifecycle.next_attempt();

        assert_eq!(unexpected_exit_attempt.generation, 10);
        assert_eq!(spawn_retry_attempt.generation, 11);
        assert_eq!(lifecycle.generation, 11);
        assert!(unexpected_exit_attempt.actions.iter().all(|action| {
            matches!(
                action,
                ChildAttemptAction::RotateCredentials
                    | ChildAttemptAction::BindEndpoint
                    | ChildAttemptAction::PublishEndpoint
                    | ChildAttemptAction::SpawnChild
            )
        }));
    }

    #[test]
    fn restart_streak_resets_after_successful_run() {
        assert_eq!(next_crash_streak(2, Duration::from_secs(10)), 3);
        assert_eq!(next_crash_streak(2, SUCCESSFUL_RUN), 1);
        assert_eq!(
            RESTART_DELAYS,
            [
                Duration::from_secs(1),
                Duration::from_secs(5),
                Duration::from_secs(30)
            ]
        );
        assert_eq!(
            crash_streak_after_exit(2, Duration::from_millis(1), ExitKind::Intentional),
            2
        );
        assert_eq!(
            crash_streak_after_exit(2, Duration::from_millis(1), ExitKind::Unexpected),
            3
        );
    }

    #[test]
    fn supervisor_state_contains_correlation_and_exit_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let previous = std::env::var(crate::observability::CORRELATION_ID_ENV).ok();
        std::env::set_var(
            crate::observability::CORRELATION_ID_ENV,
            "00000000-0000-4000-8000-000000000001",
        );
        write_state(
            &path,
            "restarting",
            2,
            None,
            Some(&ExitMetadata {
                code: Some(7),
                run_ms: 1234,
            }),
        );
        if let Some(previous) = previous {
            std::env::set_var(crate::observability::CORRELATION_ID_ENV, previous);
        } else {
            std::env::remove_var(crate::observability::CORRELATION_ID_ENV);
        }
        let state: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(
            state["correlation_id"],
            "00000000-0000-4000-8000-000000000001"
        );
        assert_eq!(state["last_exit_code"], 7);
        assert_eq!(state["last_run_ms"], 1234);
        assert_eq!(state["component"], "engine-core");
    }

    #[test]
    fn current_process_identity_matches() {
        assert!(process_matches_current_executable(std::process::id()));
    }

    #[test]
    fn cli_control_errors_are_deterministic_and_fail_closed() {
        assert_eq!(
            exit_code_for_control_error(ControlError::Unavailable),
            ExitCode::from(2)
        );
        assert_eq!(
            exit_code_for_control_error(ControlError::Unauthorized),
            ExitCode::from(1)
        );
        assert_eq!(
            exit_code_for_control_error(ControlError::OutOfOrder),
            ExitCode::from(1)
        );
        let dir = tempfile::tempdir().expect("control fixture");
        assert!(matches!(
            read_control_state(dir.path()),
            Err(ControlError::Unavailable)
        ));
    }

    #[test]
    fn cli_restart_refuses_malformed_control_state_without_connecting() {
        let dir = tempfile::tempdir().expect("control fixture");
        std::fs::write(dir.path().join(CONTROL_STATE_FILE), b"not-json").expect("malformed state");
        let previous = std::env::var("KOSMOS_DATA_DIR").ok();
        std::env::set_var("KOSMOS_DATA_DIR", dir.path());
        let result = restart_core();
        if let Some(previous) = previous {
            std::env::set_var("KOSMOS_DATA_DIR", previous);
        } else {
            std::env::remove_var("KOSMOS_DATA_DIR");
        }
        assert_eq!(result, ExitCode::from(2));
    }

    #[test]
    fn restart_and_shutdown_adapters_contain_no_pid_termination_fallback() {
        let source = include_str!("engine_supervisor.rs");
        let restart = source
            .split_once("pub fn restart_core")
            .and_then(|(_, rest)| rest.split_once("pub fn shutdown"))
            .map(|(body, _)| body)
            .expect("restart adapter source");
        let shutdown = source
            .split_once("pub fn shutdown")
            .and_then(|(_, rest)| rest.split_once("fn read_control_state"))
            .map(|(body, _)| body)
            .expect("shutdown adapter source");
        assert!(!restart.contains("kill("));
        assert!(!restart.contains("terminate"));
        assert!(!shutdown.contains("kill("));
        assert!(!shutdown.contains("terminate"));
    }

    #[tokio::test]
    async fn old_core_credential_cannot_authenticate_against_replacement_generation() {
        let (old_server, old_endpoint) = ControlServer::bind(
            "credential-rotation-session".into(),
            1,
            "uid:test".into(),
            b"controller".to_vec(),
            b"old-core-secret".to_vec(),
        )
        .await
        .expect("old control listener");
        old_server.abort();

        let (mut new_server, new_endpoint) = ControlServer::bind(
            "credential-rotation-session".into(),
            2,
            "uid:test".into(),
            b"controller".to_vec(),
            b"new-core-secret".to_vec(),
        )
        .await
        .expect("replacement control listener");
        let replacement = ControlState {
            endpoint: new_endpoint,
            supervisor_session_id: "credential-rotation-session".into(),
            child_generation: 2,
            owner_identity: "uid:test".into(),
            secret: "controller".into(),
        };

        let old_key_result = crate::engine_control::start_core_control_with_secret(
            replacement.clone(),
            b"old-core-secret".to_vec(),
        )
        .await;
        assert!(matches!(old_key_result, Err(ControlError::Unauthorized)));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), new_server.recv())
                .await
                .is_err(),
            "old generation must not reach replacement server"
        );

        let new_commands = crate::engine_control::start_core_control_with_secret(
            replacement,
            b"new-core-secret".to_vec(),
        )
        .await
        .expect("new generation credential");
        drop(new_commands);
        assert_eq!(new_server.recv().await, Some(ControlMessage::CoreReady));
        let _ = old_endpoint;
    }

    #[tokio::test]
    async fn dropping_core_receiver_closes_connection_and_clears_sender() {
        let (mut server, endpoint) = ControlServer::bind(
            "receiver-close-session".into(),
            1,
            "uid:test".into(),
            b"controller".to_vec(),
            b"core".to_vec(),
        )
        .await
        .expect("control listener");
        let state = ControlState {
            endpoint,
            supervisor_session_id: "receiver-close-session".into(),
            child_generation: 1,
            owner_identity: "uid:test".into(),
            secret: "controller".into(),
        };
        let commands =
            crate::engine_control::start_core_control_with_secret(state, b"core".to_vec())
                .await
                .expect("core ready");
        assert_eq!(server.recv().await, Some(ControlMessage::CoreReady));
        drop(commands);

        tokio::time::timeout(Duration::from_millis(250), async {
            loop {
                if server
                    .send_to_core(ControlMessage::ShutdownRequested)
                    .await
                    .is_err()
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("core connection cleanup");
    }

    #[tokio::test]
    async fn server_abort_closes_core_connection_and_receiver() {
        let (mut server, endpoint) = ControlServer::bind(
            "server-abort-session".into(),
            1,
            "uid:test".into(),
            b"controller".to_vec(),
            b"core".to_vec(),
        )
        .await
        .expect("control listener");
        let state = ControlState {
            endpoint,
            supervisor_session_id: "server-abort-session".into(),
            child_generation: 1,
            owner_identity: "uid:test".into(),
            secret: "controller".into(),
        };
        let mut commands =
            crate::engine_control::start_core_control_with_secret(state, b"core".to_vec())
                .await
                .expect("core ready");
        assert_eq!(server.recv().await, Some(ControlMessage::CoreReady));
        server.abort();
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(250), commands.recv())
                .await
                .expect("receiver close"),
            None
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn graceful_restart_requests_core_stop_before_owned_child_exit() {
        let dir = tempfile::tempdir().expect("control fixture");
        let marker = dir.path().join("stop");
        let mut child = Command::new("sh")
            .args([
                "-c",
                "while [ ! -f \"$1\" ]; do sleep 0.01; done",
                "controlled-core",
                marker.to_str().expect("marker path"),
            ])
            .spawn()
            .expect("controlled child");
        let (mut server, endpoint) = ControlServer::bind(
            "lifecycle-session".into(),
            11,
            "uid:test".into(),
            b"lifecycle-controller-secret".to_vec(),
            b"lifecycle-core-secret".to_vec(),
        )
        .await
        .expect("control listener");
        let state = ControlState {
            endpoint,
            supervisor_session_id: "lifecycle-session".into(),
            child_generation: 11,
            owner_identity: "uid:test".into(),
            secret: "lifecycle-controller-secret".into(),
        };
        let mut core_commands = crate::engine_control::start_core_control_with_secret(
            state.clone(),
            b"lifecycle-core-secret".to_vec(),
        )
        .await
        .expect("core ready");
        assert_eq!(server.recv().await, Some(ControlMessage::CoreReady));
        let waiter = tokio::spawn(async move {
            let result = wait_for_child(&mut child, &mut server).await;
            assert_eq!(
                result,
                ChildResult::Control(ControlMessage::RestartRequested)
            );
            assert_eq!(
                core_commands.recv().await,
                Some(ControlMessage::RestartRequested)
            );
            std::fs::write(&marker, "stopping").expect("graceful marker");
            stop_owned_child(&mut child).await;
            child
                .try_wait()
                .expect("child status")
                .expect("child exited")
        });
        crate::engine_control::send_control(&state, ControlMessage::RestartRequested)
            .await
            .expect("restart request");
        let status = waiter.await.expect("supervisor waiter");
        assert!(status.success());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn graceful_shutdown_requests_core_stop_and_leaves_no_child() {
        let dir = tempfile::tempdir().expect("control fixture");
        let marker = dir.path().join("stop");
        let mut child = Command::new("sh")
            .args([
                "-c",
                "while [ ! -f \"$1\" ]; do sleep 0.01; done",
                "controlled-core",
                marker.to_str().expect("marker path"),
            ])
            .spawn()
            .expect("controlled child");
        let (mut server, endpoint) = ControlServer::bind(
            "shutdown-session".into(),
            12,
            "uid:test".into(),
            b"shutdown-controller-secret".to_vec(),
            b"shutdown-core-secret".to_vec(),
        )
        .await
        .expect("control listener");
        let state = ControlState {
            endpoint,
            supervisor_session_id: "shutdown-session".into(),
            child_generation: 12,
            owner_identity: "uid:test".into(),
            secret: "shutdown-controller-secret".into(),
        };
        let mut core_commands = crate::engine_control::start_core_control_with_secret(
            state.clone(),
            b"shutdown-core-secret".to_vec(),
        )
        .await
        .expect("core ready");
        assert_eq!(server.recv().await, Some(ControlMessage::CoreReady));
        let waiter = tokio::spawn(async move {
            let result = wait_for_child(&mut child, &mut server).await;
            assert_eq!(
                result,
                ChildResult::Control(ControlMessage::ShutdownRequested)
            );
            assert_eq!(
                core_commands.recv().await,
                Some(ControlMessage::ShutdownRequested)
            );
            std::fs::write(&marker, "stopping").expect("graceful marker");
            stop_owned_child(&mut child).await;
            child
                .try_wait()
                .expect("child status")
                .expect("child exited")
        });
        crate::engine_control::send_control(&state, ControlMessage::ShutdownRequested)
            .await
            .expect("shutdown request");
        let status = waiter.await.expect("supervisor waiter");
        assert!(status.success());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn closed_core_channel_waits_for_deadline_before_force_kill() {
        let mut child = Command::new("sleep")
            .arg("10")
            .spawn()
            .expect("controlled child");
        let (mut server, _) = ControlServer::bind(
            "closed-channel-session".into(),
            1,
            "uid:test".into(),
            b"controller".to_vec(),
            b"core".to_vec(),
        )
        .await
        .expect("control listener");
        server.abort();
        let started = Instant::now();
        stop_owned_child_gracefully_with_deadline(
            &mut child,
            &mut server,
            Duration::from_millis(40),
        )
        .await;
        assert!(started.elapsed() >= Duration::from_millis(30));
        assert!(child.try_wait().expect("child status").is_some());
    }

    #[tokio::test]
    async fn deadline_force_kills_only_the_owned_child_handle() {
        #[cfg(unix)]
        let mut command = {
            let mut command = Command::new("sleep");
            command.arg("10");
            command
        };
        #[cfg(windows)]
        let mut command = {
            let mut command = Command::new("cmd.exe");
            command.args(["/D", "/C", "ping -n 11 127.0.0.1 >NUL"]);
            command.creation_flags(CREATE_NO_WINDOW);
            command
        };
        let mut child = command.spawn().expect("controlled child");
        stop_owned_child_with_deadline(&mut child, Duration::from_millis(10)).await;
        assert!(child.try_wait().expect("child status").is_some());
    }

    #[test]
    fn sync_env_loads_only_runtime_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("sync.env"),
            "KOSMOS_SPACE_ID = personal\nIGNORED_SECRET=nope\n# KOSMOS_IROH=0\n",
        )
        .expect("write sync.env");

        assert_eq!(
            read_sync_env(dir.path()),
            vec![("KOSMOS_SPACE_ID".to_string(), "personal".to_string())]
        );
    }

    #[test]
    #[cfg(unix)]
    fn raw_control_state_is_owner_only_and_removed_on_terminal_drop() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("control fixture");
        let path = dir.path().join(CONTROL_STATE_FILE);
        let state = ControlState {
            endpoint: "127.0.0.1:1".into(),
            supervisor_session_id: "session".into(),
            child_generation: 1,
            owner_identity: "uid:test".into(),
            secret: "raw-secret-must-not-leak".into(),
        };
        lock_file::write_owner_only_json(&path, &state).expect("protected control state");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains(&state.secret));
        {
            let _cleanup = ControlStateCleanup(&path);
        }
        assert!(!path.exists());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn spontaneous_success_exit_uses_unexpected_backoff_path() {
        let mut child = Command::new("sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("controlled core");
        let started = Instant::now();
        let status = child.wait().await.expect("child exit");
        assert!(status.success());
        assert_eq!(
            crash_streak_after_exit(0, started.elapsed(), ExitKind::Unexpected),
            1
        );
    }

    #[test]
    fn spontaneous_success_exit_is_unexpected() {
        assert_eq!(
            crash_streak_after_exit(0, Duration::from_millis(1), ExitKind::Unexpected),
            1
        );
    }
    #[test]
    fn core_worker_spawn_keeps_windows_console_hidden() {
        // Regression: 2026-07-31. Supervised core worker briefly opened a console on Windows.
        let source = include_str!("engine_supervisor.rs");
        let spawn = source
            .split_once("fn spawn_core_worker")
            .and_then(|(_, rest)| rest.split_once("fn read_sync_env"))
            .map(|(body, _)| body)
            .expect("spawn_core_worker source");
        assert!(spawn.contains("creation_flags(CREATE_NO_WINDOW)"));
    }
}
