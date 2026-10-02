use super::*;

pub(crate) fn running_engine_pid(lock_path: &Path) -> Option<u32> {
    let lock = lock_file::read_engine(lock_path).ok()?;
    process_matches_current_executable(lock.pid).then_some(lock.pid)
}

pub(crate) enum MonitorResult {
    Shutdown,
    Exited { ran_for: Duration },
}

pub(crate) async fn monitor_existing_core(lock_path: &Path) -> MonitorResult {
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
pub(super) enum ChildResult {
    Shutdown,
    Control(ControlMessage),
    Exited(std::process::ExitStatus),
}

pub(super) async fn wait_for_child(child: &mut Child, control: &mut ControlServer) -> ChildResult {
    loop {
        tokio::select! {
            result = child.wait() => {
                return match result {
                    Ok(status) => ChildResult::Exited(status),
                    Err(error) => {
                        crate::observability::stderr(format!(
                            "[mundus-engine] wait for core failed: {error}"
                        ));
                        ChildResult::Exited(exit_status_failure())
                    }
                };
            }
            message = control.recv() => {
                match message {
                    Some(
                        ControlMessage::RestartRequested
                    ) | Some(ControlMessage::ShutdownRequested) => {
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
pub(crate) async fn stop_owned_child(child: &mut Child) {
    stop_owned_child_with_deadline(child, GRACEFUL_STOP_DEADLINE).await;
}

pub(crate) async fn stop_owned_child_gracefully(child: &mut Child, control: &mut ControlServer) {
    stop_owned_child_gracefully_with_deadline(child, control, GRACEFUL_STOP_DEADLINE).await;
}

pub(crate) async fn stop_owned_child_gracefully_with_deadline(
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

pub(crate) async fn stop_owned_child_with_deadline(child: &mut Child, deadline: Duration) {
    let exited = tokio::time::timeout(deadline, child.wait()).await;
    if exited.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}
