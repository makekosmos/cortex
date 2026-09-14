use super::*;

pub const CORE_WORKER_ARG: &str = "--core-worker";
pub const RESTART_CORE_ARG: &str = "--restart-core";
pub const SHUTDOWN_ARG: &str = "--shutdown";
pub const START_ARG: &str = "--start";
/// Exit reason propagated from the backend tray to the Electron owner.
pub const TRAY_EXIT_CODE: u8 = 42;

pub(crate) const RESTART_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(5),
    Duration::from_secs(30),
];
pub(crate) const SUCCESSFUL_RUN: Duration = Duration::from_secs(5 * 60);
pub(crate) const GRACEFUL_STOP_DEADLINE: Duration = Duration::from_secs(5);
pub(crate) const SUPERVISOR_LOCK_FILE: &str = "engine-supervisor.lock.db";
pub(crate) const SUPERVISOR_STATE_FILE: &str = "engine-supervisor-state.json";
pub(crate) const CONTROL_STATE_FILE: &str = "engine-supervisor-control.json";
pub(crate) const SYNC_ENV_KEYS: &[&str] = &[
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
pub(crate) struct SupervisorState<'a> {
    pub(crate) format_version: u32,
    pub(crate) component: &'static str,
    pub(crate) version: &'static str,
    pub(crate) correlation_id: String,
    pub(crate) supervisor_pid: u32,
    pub(crate) core_pid: Option<u32>,
    pub(crate) status: &'a str,
    pub(crate) crash_streak: usize,
    pub(crate) last_exit_code: Option<i32>,
    pub(crate) last_run_ms: Option<u64>,
    pub(crate) updated_at: String,
}

pub(crate) struct ControlStateCleanup<'a>(pub(crate) &'a Path);

impl Drop for ControlStateCleanup<'_> {
    fn drop(&mut self) {
        cleanup_state(self.0);
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ExitMetadata {
    pub(crate) code: Option<i32>,
    pub(crate) run_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChildAttemptAction {
    RotateCredentials,
    BindEndpoint,
    PublishEndpoint,
    SpawnChild,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ChildAttempt {
    pub(crate) generation: u64,
    pub(crate) actions: [ChildAttemptAction; 4],
}

pub(crate) struct ChildLifecycle {
    pub(crate) generation: u64,
}

impl ChildLifecycle {
    pub(crate) fn new(generation: u64) -> Self {
        Self { generation }
    }

    pub(crate) fn next_attempt(&mut self) -> ChildAttempt {
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

pub(crate) enum SpawnAttemptError {
    Control,
    State,
    Spawn(std::io::Error),
}
