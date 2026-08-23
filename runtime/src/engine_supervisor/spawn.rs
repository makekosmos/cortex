use super::*;

pub(crate) fn spawn_core_worker(
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
    // РЎРј. postmortems.md В§ 2026-07-31: supervised workers must stay background-only.
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command.spawn()
}

#[cfg(windows)]
pub(super) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) async fn spawn_child_attempt(
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

pub(crate) fn read_sync_env(data_dir: &Path) -> Vec<(String, String)> {
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
