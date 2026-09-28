use super::*;
use crate::engine_control::ControlError;

#[test]
fn supervisor_uses_only_engine_lock_contract() {
    let source = include_str!("supervisor.rs");
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
    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
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
    let previous = std::env::var("MUNDUS_DATA_DIR").ok();
    std::env::set_var("MUNDUS_DATA_DIR", dir.path());
    let result = restart_core();
    if let Some(previous) = previous {
        std::env::set_var("MUNDUS_DATA_DIR", previous);
    } else {
        std::env::remove_var("MUNDUS_DATA_DIR");
    }
    assert_eq!(result, ExitCode::from(2));
}

#[test]
fn core_worker_spawn_keeps_windows_console_hidden() {
    // Regression: 2026-07-31. Supervised core worker briefly opened a console on Windows.
    let source = include_str!("spawn.rs");
    let spawn = source
        .split_once("fn spawn_core_worker")
        .and_then(|(_, rest)| rest.split_once("fn read_sync_env"))
        .map(|(body, _)| body)
        .expect("spawn_core_worker source");
    assert!(spawn.contains("creation_flags(CREATE_NO_WINDOW)"));
}
