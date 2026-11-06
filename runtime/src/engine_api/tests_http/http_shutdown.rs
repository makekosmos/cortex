use super::*;
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
pub(super) async fn dropped_api_server_releases_package_service_back_reference() {
    // PackageService stores its package-definition dispatcher
    // as a Weak ref — a server dropped without shutdown() must not keep
    // the dispatcher (and every component its dispatch closure captures)
    // alive. The fixture dir is removable immediately after drop.
    let dir = tempfile::tempdir().expect("fixture dir");
    let ark = Arc::new(
        crate::ark_host::ArkHost::open(&dir.path().join("ark.db").to_string_lossy())
            .await
            .expect("ark host fixture"),
    );
    let app_index = Arc::new(
        crate::app_index::AppIndex::new(dir.path(), dir.path().join("icons")).expect("app index"),
    );
    let file_index =
        Arc::new(crate::file_index::FileIndex::new_disabled(dir.path()).expect("file index"));
    let package_service =
        Arc::new(crate::package_service::PackageService::open(dir.path()).expect("packages"));
    let usage =
        Arc::new(crate::protocol_usage::ProtocolUsageStore::open(dir.path()).expect("usage"));
    let token = "a".repeat(64);
    let ws = crate::ws_server::WsServer::bind(
        ark.clone(),
        token.clone(),
        dir.path().to_path_buf(),
        app_index,
        file_index,
        Arc::new(crate::usage_tracker::UsageTrackerDiagnosticsState::default()),
        usage.clone(),
        package_service.clone(),
        "00000000-0000-4000-8000-000000000001".into(),
    )
    .await
    .expect("ws bind");
    let api = EngineApiServer::bind(
        token.clone(),
        ws.port(),
        usage,
        "00000000-0000-4000-8000-000000000001".into(),
        package_service,
        Arc::new(ws.dispatcher()),
    )
    .await
    .expect("http bind");
    drop(api);
    drop(ws);
    let ark = Arc::try_unwrap(ark).unwrap_or_else(|_| panic!("dropped servers released ArkHost"));
    ark.shutdown(std::time::Instant::now() + Duration::from_secs(30))
        .await
        .expect("ARK closes before fixture cleanup");
    dir.close()
        .expect("fixture dir still locked after dropping servers");
}

#[tokio::test]
pub(super) async fn shutdown_handle_aborts_stalled_operation_and_releases_ownership_once() {
    let cleanups = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
        Arc::new(|_| {
            Box::pin(async {
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(json!({"never": "returned"}))
            })
        }),
        Arc::new({
            let cleanups = cleanups.clone();
            move |_| {
                let cleanups = cleanups.clone();
                cleanups.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }),
    ));
    let token = "a".repeat(64);
    let (_fixture_dir, server) = EngineApiServer::bind_with_test_dispatcher(
        token,
        dispatcher.clone(),
        Duration::from_secs(30),
    )
    .await
    .expect("server");
    let owner = dispatcher.allocate_owner().unwrap();
    let owner_id = owner.id();

    let request =
        crate::engine_dispatch::DispatchRequest::from_wire(json!({"operation":"stalled.write"}))
            .expect("request")
            .with_client(crate::engine_dispatch::DispatchClient {
                connection_id: Some(owner_id),
                desktop_authorized: false,
                ..Default::default()
            });
    let _response = server.operations.start(request, dispatcher, owner).await;
    let handle = server.shutdown_handle();
    let _ = tokio::time::timeout(Duration::from_secs(60), handle.shutdown())
        .await
        .expect("shutdown deadline");
    assert_eq!(cleanups.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(server.operations.closed.load(Ordering::Acquire));
}

#[tokio::test]
pub(super) async fn shutdown_closes_admission_and_completes_synchronous_cleanup() {
    let cleanup_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
        Arc::new(|_| Box::pin(async { Ok(json!({"ok": true})) })),
        Arc::new({
            let cleanup_count = cleanup_count.clone();
            move |_| {
                cleanup_count.fetch_add(1, Ordering::SeqCst);
            }
        }),
    ));
    let (_fixture_dir, server) = EngineApiServer::bind_with_test_dispatcher(
        "a".repeat(64),
        dispatcher.clone(),
        Duration::from_secs(60),
    )
    .await
    .expect("server");
    let owner = dispatcher.allocate_owner().unwrap();
    let owner_id = owner.id();
    let request = crate::engine_dispatch::DispatchRequest::from_wire(
        json!({"operation":"controlled.cleanup"}),
    )
    .expect("request")
    .with_client(crate::engine_dispatch::DispatchClient {
        connection_id: Some(owner_id),
        desktop_authorized: false,
        ..Default::default()
    });
    assert!(server
        .operations
        .start(request, dispatcher.clone(), owner)
        .await
        .is_some());
    server.shutdown_handle().shutdown().await.expect("shutdown");
    assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
    assert!(server.operations.closed.load(Ordering::Acquire));
    assert!(server.operations.permits.available_permits() > 0);
}

#[tokio::test]
pub(super) async fn http_dispatch_starts_only_after_registry_install() {
    let registry = HttpOperationRegistry::default();
    let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed_registry = registry.clone();
    let observed_started = started.clone();
    let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
        move |_| {
            let installed = !observed_registry
                .continuations
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_empty();
            observed_started.store(installed, Ordering::SeqCst);
            Box::pin(async { Ok(json!({"ok": true})) })
        },
    )));
    let owner = dispatcher.allocate_owner().unwrap();
    let request =
        crate::engine_dispatch::DispatchRequest::from_wire(json!({"operation":"install-gated"}))
            .unwrap()
            .with_client(crate::engine_dispatch::DispatchClient {
                connection_id: Some(owner.id()),
                ..Default::default()
            });

    let (_, receiver, _guard) = registry
        .start(request, dispatcher, owner)
        .await
        .expect("operation installed");
    assert!(receiver.await.unwrap().is_ok());
    assert!(started.load(Ordering::SeqCst));
}

#[tokio::test]
pub(super) async fn admission_racing_shutdown_cannot_insert_after_drain() {
    let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
        |_| Box::pin(async { Ok(json!({"ok": true})) }),
    )));
    let (_fixture_dir, server) = EngineApiServer::bind_with_test_dispatcher(
        "a".repeat(64),
        dispatcher.clone(),
        Duration::from_secs(60),
    )
    .await
    .expect("server");
    let mut admissions = Vec::new();
    for _ in 0..64 {
        let registry = server.operations.clone();
        let dispatcher = dispatcher.clone();
        admissions.push(tokio::spawn(async move {
            let owner = dispatcher.allocate_owner().unwrap();
            let owner_id = owner.id();
            let request =
                crate::engine_dispatch::DispatchRequest::from_wire(json!({"operation":"race"}))
                    .expect("request")
                    .with_client(crate::engine_dispatch::DispatchClient {
                        connection_id: Some(owner_id),
                        desktop_authorized: false,
                        ..Default::default()
                    });
            registry.start(request, dispatcher, owner).await.is_some()
        }));
    }
    let shutdown_handle = server.shutdown_handle();
    let shutdown = tokio::spawn(async move { shutdown_handle.shutdown().await });
    for admission in admissions {
        let _ = admission.await.expect("admission task");
    }
    let _ = shutdown.await.expect("shutdown task");
    assert!(server.operations.closed.load(Ordering::Acquire));
    assert!(server.operations.continuations.lock().unwrap().is_empty());
    assert_eq!(dispatcher.live_owner_count(), 0);
}
