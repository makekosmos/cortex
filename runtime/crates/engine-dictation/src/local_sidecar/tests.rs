use std::env;

use super::*;

static ENV_SIDECAR_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn warm_test_service(timeout_ms: u64) -> LocalSttSidecarService {
    let mut service = LocalSttSidecarService::new();
    service.state = WarmState {
        loaded_model: Some(LocalSttModelSpec {
            engine: "whisper.cpp".into(),
            model_id: Some("turbo".into()),
            model_path: Some("C:/models/turbo.bin".into()),
            command_path: Some("C:/tools/whisper.cpp-vulkan/Release/whisper-cli.exe".into()),
            accelerator: LocalSttAccelerator::Gpu,
            profile: LocalSttProfile::Accurate,
            idle_unload_after_ms: Some(timeout_ms),
        }),
        backend: Some("whisper_server".into()),
        last_used_at: Some(Instant::now()),
        embedded: None,
    };
    service
}

#[tokio::test]
async fn status_exposes_accelerator_device_profile_and_idle_timeout() {
    let mut service = warm_test_service(300_000);
    let response = service
        .handle(LocalSttRequestEnvelope {
            request_id: 1,
            request: LocalSttRequest::Status,
        })
        .await;

    let LocalSttResponse::Status(status) = response.response.expect("expected status response")
    else {
        panic!("expected status response");
    };
    assert!(status.warm);
    assert_eq!(status.accelerator, LocalSttAccelerator::Gpu);
    assert_eq!(status.profile, LocalSttProfile::Accurate);
    assert_eq!(status.idle_unload_after_ms, Some(300_000));
}

#[tokio::test]
async fn cancel_unloads_backend_and_reports_accepted() {
    let mut service = warm_test_service(300_000);
    let response = service
        .handle(LocalSttRequestEnvelope {
            request_id: 2,
            request: LocalSttRequest::Cancel {
                target_request_id: None,
            },
        })
        .await;

    let LocalSttResponse::Ack(ack) = response.response.expect("expected ack response") else {
        panic!("expected ack response");
    };
    assert!(ack.accepted);
    assert!(!service.status().warm);
}

#[tokio::test]
async fn idle_watcher_unloads_without_new_request() {
    let _guard = ENV_SIDECAR_LOCK.lock().await;
    unsafe {
        env::set_var("MUNDUS_LOCAL_STT_IDLE_WATCH_INTERVAL_MS", "10");
    }
    let service = Arc::new(tokio::sync::Mutex::new(warm_test_service(1)));
    {
        let mut service = service.lock().await;
        service.state.last_used_at = Some(Instant::now() - Duration::from_millis(20));
    }
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let watcher = spawn_idle_unload_watcher(Arc::clone(&service), Arc::clone(&shutdown_signal));

    tokio::time::sleep(Duration::from_millis(50)).await;

    shutdown_signal.store(true, Ordering::Relaxed);
    watcher.abort();
    unsafe {
        env::remove_var("MUNDUS_LOCAL_STT_IDLE_WATCH_INTERVAL_MS");
    }
    assert!(!service.lock().await.status().warm);
}

#[tokio::test]
async fn status_unloads_after_idle_timeout() {
    let mut service = warm_test_service(1);
    service.state.last_used_at = Some(Instant::now() - Duration::from_millis(5));

    let response = service
        .handle(LocalSttRequestEnvelope {
            request_id: 3,
            request: LocalSttRequest::Status,
        })
        .await;

    let LocalSttResponse::Status(status) = response.response.expect("expected status response")
    else {
        panic!("expected status response");
    };
    assert!(!status.warm);
    assert!(status.loaded_model.is_none());
}
