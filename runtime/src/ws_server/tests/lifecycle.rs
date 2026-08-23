use super::*;
#[tokio::test]
async fn shutdown_reports_deadline_breach_after_forced_reap_and_is_idempotent() {
    let handle = WsShutdownHandle {
        lifecycle: Arc::new(WsLifecycle::default()),
    };
    let task = tokio::spawn(async {
        std::future::pending::<()>().await;
    });
    handle
        .lifecycle
        .tasks
        .lock()
        .unwrap()
        .insert(1, WsConnectionSlot::Installed(task));

    let started = Instant::now();
    assert!(handle.drain(Duration::from_millis(5)).await.is_err());
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(handle.task_count(), 0);
    assert!(handle.shutdown().await.is_ok());
}

#[tokio::test]
async fn ws_reserved_shutdown_releases_socket_owner_and_permit_before_drain_returns() {
    let handle = WsShutdownHandle {
        lifecycle: Arc::new(WsLifecycle::default()),
    };
    let allocator = crate::engine_dispatch::OwnerAllocator::default();
    let owner = allocator.allocate().unwrap();
    let permit = handle
        .lifecycle
        .capacity
        .clone()
        .try_acquire_owned()
        .unwrap();
    let (start, _started) = tokio::sync::oneshot::channel();
    handle.lifecycle.tasks.lock().unwrap().insert(
        1,
        WsConnectionSlot::Reserved {
            resources: Arc::new(Mutex::new(Some(WsConnectionResources {
                stream: None,
                permit: Some(permit),
                owner_lease: Some(owner),
            }))),
            start,
        },
    );

    handle.shutdown().await.unwrap();

    assert_eq!(handle.task_count(), 0);
    assert_eq!(handle.available_capacity(), MAX_ACTIVE_WS_CONNECTIONS);
    assert_eq!(allocator.live_count(), 0);
}

#[tokio::test]
async fn ws_preinstall_failure_drops_reserved_resources_without_spawn() {
    let handle = WsShutdownHandle {
        lifecycle: Arc::new(WsLifecycle::default()),
    };
    let allocator = crate::engine_dispatch::OwnerAllocator::default();
    let owner = allocator.allocate().unwrap();
    let permit = handle
        .lifecycle
        .capacity
        .clone()
        .try_acquire_owned()
        .unwrap();
    let (start, _started) = tokio::sync::oneshot::channel();
    handle.lifecycle.tasks.lock().unwrap().insert(
        1,
        WsConnectionSlot::Reserved {
            resources: Arc::new(Mutex::new(Some(WsConnectionResources {
                stream: None,
                permit: Some(permit),
                owner_lease: Some(owner),
            }))),
            start,
        },
    );
    let slot = handle.lifecycle.tasks.lock().unwrap().remove(&1).unwrap();
    drop(slot);

    assert_eq!(handle.task_count(), 0);
    assert_eq!(handle.available_capacity(), MAX_ACTIVE_WS_CONNECTIONS);
    assert_eq!(allocator.live_count(), 0);
}

#[tokio::test]
async fn ws_shutdown_waits_for_reserved_admission_before_closing_and_draining() {
    let handle = WsShutdownHandle {
        lifecycle: Arc::new(WsLifecycle::default()),
    };
    let admission = handle.lifecycle.admission.lock().unwrap();
    let lifecycle = handle.lifecycle.clone();
    let shutdown = std::thread::spawn(move || {
        let _admission = lifecycle.admission.lock().unwrap();
        lifecycle.closed.store(true, Ordering::Release);
    });
    std::thread::yield_now();
    assert!(!handle.lifecycle.closed.load(Ordering::Acquire));
    drop(admission);
    shutdown.join().unwrap();
    handle.shutdown().await.unwrap();
    assert!(handle.lifecycle.closed.load(Ordering::Acquire));
    assert_eq!(handle.task_count(), 0);
    assert_eq!(handle.request_task_count(), 0);
}

#[tokio::test]
async fn ws_immediate_connection_tasks_are_joined_and_reaped() {
    let handle = WsShutdownHandle {
        lifecycle: Arc::new(WsLifecycle::default()),
    };
    let task = tokio::spawn(async {});
    handle
        .lifecycle
        .tasks
        .lock()
        .unwrap()
        .insert(1, WsConnectionSlot::Installed(task));
    tokio::task::yield_now().await;
    handle.reap().await;
    assert_eq!(handle.task_count(), 0);
}

#[tokio::test]
async fn ws_connection_registry_stays_bounded_under_10k_immediate_accept_close_cycles() {
    let handle = WsShutdownHandle {
        lifecycle: Arc::new(WsLifecycle::default()),
    };
    for id in 1..=10_000 {
        let task = tokio::spawn(async {});
        handle
            .lifecycle
            .tasks
            .lock()
            .unwrap()
            .insert(id, WsConnectionSlot::Installed(task));
        tokio::task::yield_now().await;
        handle.reap().await;
        assert!(handle.task_count() <= 1);
    }
    assert_eq!(handle.task_count(), 0);
}

#[tokio::test]
async fn shutdown_admission_race_cannot_spawn_after_request_drain() {
    let handle = WsShutdownHandle {
        lifecycle: Arc::new(WsLifecycle::default()),
    };
    handle.shutdown().await.unwrap();
    let spawned = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let spawned_clone = spawned.clone();
    let permit = Arc::new(Mutex::new(Some(
        handle
            .lifecycle
            .request_capacity
            .clone()
            .try_acquire_owned()
            .unwrap(),
    )));
    let (cancel_sender, _cancel_receiver) = tokio::sync::oneshot::channel();
    let installed = handle.install_request(
        1,
        permit,
        Arc::new(Mutex::new(Some(cancel_sender))),
        move |start_receiver| {
            spawned_clone.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let _ = start_receiver.await;
            })
        },
    );
    assert!(!installed);
    assert_eq!(spawned.load(Ordering::SeqCst), 0);
    assert_eq!(handle.request_task_count(), 0);
    assert_eq!(
        handle.lifecycle.request_capacity.available_permits(),
        MAX_ACTIVE_WS_REQUESTS
    );
}
