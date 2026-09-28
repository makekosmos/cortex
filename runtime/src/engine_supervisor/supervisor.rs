use super::*;

pub async fn run_supervisor() -> ExitCode {
    let mut pending_desktop_lease = match take_desktop_lease_env() {
        Ok(value) => value,
        Err(()) => return ExitCode::from(2),
    };
    let data_dir = match lock_file::mundus_data_dir() {
        Ok(path) => path,
        Err(error) => {
            crate::observability::stderr(format!(
                "[mundus-engine] failed to resolve data dir: {error}"
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
                "[mundus-engine] supervisor singleton failed: {error}"
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
            "[mundus-engine] supervisor unavailable: existing core has no verifiable control ownership",
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
                        "[mundus-engine] cannot stop an existing core without authenticated ownership",
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
                        "[mundus-engine] failed to spawn core worker: {}",
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
                    if status.code() == Some(TRAY_EXIT_CODE as i32) {
                        cleanup_state(&state_path);
                        cleanup_state(&control_state_path);
                        return ExitCode::from(TRAY_EXIT_CODE);
                    }
                    let ran_for = started_at.elapsed();
                    last_exit = Some(ExitMetadata {
                        code: status.code(),
                        run_ms: duration_ms(ran_for),
                    });
                    crate::observability::stderr(format!(
                        "[mundus-engine] core exited with {status} after {:.1}s",
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
                "[mundus-engine] core failed {} times; automatic restart stopped",
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
