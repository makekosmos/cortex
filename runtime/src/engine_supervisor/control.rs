use super::*;

const DESKTOP_LEASE_READY_TIMEOUT: Duration = Duration::from_secs(30);

pub fn restart_core() -> ExitCode {
    let data_dir = match lock_file::mundus_data_dir() {
        Ok(path) => path,
        Err(error) => {
            crate::observability::stderr(format!(
                "[mundus-engine] restart failed to resolve data dir: {error}"
            ));
            return ExitCode::from(1);
        }
    };
    match read_control_state(&data_dir).and_then(|state| {
        engine_control::send_control_blocking(&state, ControlMessage::RestartRequested)
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            crate::observability::stderr(format!("[mundus-engine] restart refused: {error}"));
            exit_code_for_control_error(error)
        }
    }
}

pub fn shutdown() -> ExitCode {
    let data_dir = match lock_file::mundus_data_dir() {
        Ok(path) => path,
        Err(error) => {
            crate::observability::stderr(format!(
                "[mundus-engine] shutdown failed to resolve data dir: {error}"
            ));
            return ExitCode::from(1);
        }
    };
    match read_control_state(&data_dir).and_then(|state| {
        engine_control::send_control_blocking(&state, ControlMessage::ShutdownRequested)
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            crate::observability::stderr(format!("[mundus-engine] shutdown refused: {error}"));
            exit_code_for_control_error(error)
        }
    }
}

pub(crate) async fn send_desktop_lease_when_ready(
    control_server: &mut ControlServer,
    electron_pid: u32,
    credential: String,
) -> Result<(), engine_control::ControlError> {
    // Engine cold start includes ARK/SQLite initialization and can exceed two
    // seconds on a real user profile. Keep a bounded wait, but do not kill a
    // healthy child before it reaches its authenticated control phase.
    let deadline = Instant::now() + DESKTOP_LEASE_READY_TIMEOUT;
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
                                "[mundus-engine] desktop lease acknowledgement failed: {error}"
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

pub(crate) fn read_control_state(
    data_dir: &Path,
) -> Result<ControlState, engine_control::ControlError> {
    let path = data_dir.join(CONTROL_STATE_FILE);
    let bytes = std::fs::read(path).map_err(|_| engine_control::ControlError::Unavailable)?;
    serde_json::from_slice(&bytes).map_err(|_| engine_control::ControlError::Unavailable)
}

pub(crate) fn exit_code_for_control_error(error: engine_control::ControlError) -> ExitCode {
    match error {
        engine_control::ControlError::Unavailable => ExitCode::from(2),
        _ => ExitCode::from(1),
    }
}

pub(crate) fn current_owner_identity() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".into())
}
