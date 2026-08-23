use super::super::*;

#[cfg(windows)]
#[test]
fn windows_start_transaction_signature_is_compile_checked() {
    #[allow(clippy::too_many_arguments)]
    fn call_chain(
        supervisor: &PackageWorkerSupervisor,
        key: (String, String),
        spec: LaunchSpec,
        generation: u64,
        restart_count: u32,
        failure_streak: u8,
        restart_allowed: bool,
        cancel_rx: oneshot::Receiver<()>,
        owner: Arc<LaunchCleanupOwner>,
        process_holder: WorkerProcessHolder,
    ) -> impl std::future::Future<Output = Result<(), &'static str>> + '_ {
        supervisor.start_windows_transaction(
            key,
            spec,
            generation,
            restart_count,
            failure_streak,
            restart_allowed,
            cancel_rx,
            owner,
            process_holder,
        )
    }
    let _ = call_chain;
}

#[tokio::test]
async fn task_registry_publishes_before_start_and_drains_cancelled_tasks() {
    let registry = TaskRegistry::new(1);
    let key = TaskKey::Call {
        package: "pkg".into(),
        version: "1".into(),
        generation: 7,
        id: "call-1".into(),
    };
    let (start_rx, _cancel_rx) = registry.reserve(key.clone()).expect("reservation");
    let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let started_task = started.clone();
    let task = tokio::spawn(async move {
        if !start_rx.await.unwrap_or(false) {
            return;
        }
        started_task.store(true, Ordering::SeqCst);
        std::future::pending::<()>().await;
    });
    assert!(!started.load(Ordering::SeqCst));
    assert!(registry.install(&key, task).is_ok());
    tokio::task::yield_now().await;
    assert!(started.load(Ordering::SeqCst));
    assert_eq!(registry.len(), 1);
    assert!(!registry.shutdown().await);
    assert_eq!(registry.len(), 0);
}

#[tokio::test]
async fn worker_pipe_tasks_are_installed_before_any_gate_opens() {
    let registry = TaskRegistry::new(3);
    let keys = [
        TaskKey::WorkerStdin {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        },
        TaskKey::WorkerStdout {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        },
        TaskKey::WorkerStderr {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        },
    ];
    let mut gates = Vec::new();
    for key in &keys {
        gates.push((
            key.clone(),
            registry.reserve(key.clone()).expect("reservation").0,
        ));
    }
    let started = (0..3)
        .map(|_| Arc::new(std::sync::atomic::AtomicBool::new(false)))
        .collect::<Vec<_>>();
    let mut tasks = Vec::new();
    for (index, (key, gate)) in gates.into_iter().enumerate() {
        let marker = started[index].clone();
        let task = tokio::spawn(async move {
            if gate.await.unwrap_or(false) {
                marker.store(true, Ordering::SeqCst);
            }
        });
        registry.install_pending(&key, task).expect("install");
        tasks.push(key);
    }
    tokio::task::yield_now().await;
    assert!(started.iter().all(|marker| !marker.load(Ordering::SeqCst)));
    assert!(registry.open(&tasks[0]));
    tokio::task::yield_now().await;
    assert!(started[0].load(Ordering::SeqCst));
    assert!(!started[1].load(Ordering::SeqCst));
    assert!(!started[2].load(Ordering::SeqCst));
    assert!(registry.open(&tasks[1]));
    assert!(registry.open(&tasks[2]));
    tokio::task::yield_now().await;
    assert!(started.iter().all(|marker| marker.load(Ordering::SeqCst)));
    registry.reap_completed().await;
    assert_eq!(registry.len(), 0);
    assert!(registry.shutdown().await);
}

#[tokio::test]
async fn owned_registry_keeps_completed_calls_until_explicit_reap() {
    let registry = TaskRegistry::owned(1);
    let key = TaskKey::Call {
        package: "pkg".into(),
        version: "1".into(),
        generation: 1,
        id: "call-1".into(),
    };
    let (start_rx, _cancel_rx) = registry.reserve(key.clone()).expect("reservation");
    let task = tokio::spawn(async move {
        assert!(start_rx.await.expect("start gate"));
    });
    registry.install(&key, task).expect("install");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(registry.len(), 1);
    registry.reap_completed().await;
    assert_eq!(registry.len(), 0);
    assert!(registry.reserve(key).is_some());
}

#[tokio::test]
async fn task_registry_reaps_10000_completed_tasks_without_background_work() {
    let registry = TaskRegistry::owned(1);
    for id in 0..10_000 {
        let key = TaskKey::Call {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
            id: id.to_string(),
        };
        let (start_rx, _cancel_rx) = registry.reserve(key.clone()).expect("reservation");
        let task = tokio::spawn(async move {
            assert!(start_rx.await.expect("start gate"));
        });
        registry.install(&key, task).expect("install");
        tokio::task::yield_now().await;
        registry.reap_completed().await;
    }
    assert_eq!(registry.len(), 0);
    assert!(registry.shutdown().await);
}

#[tokio::test]
async fn shutdown_aborts_pending_task_and_joins_it() {
    let registry = TaskRegistry::owned(1);
    let key = TaskKey::Call {
        package: "pkg".into(),
        version: "1".into(),
        generation: 1,
        id: "pending".into(),
    };
    let (start_rx, _cancel_rx) = registry.reserve(key.clone()).expect("reservation");
    let task = tokio::spawn(async move {
        assert!(start_rx.await.expect("start gate"));
        std::future::pending::<()>().await;
    });
    registry.install(&key, task).expect("install");

    assert!(!registry.shutdown().await);
    assert_eq!(registry.len(), 0);
}

#[tokio::test]
async fn startup_timeout_quarantines_without_aborting_and_reaps_after_release() {
    let registry = TaskRegistry::owned(1);
    let key = TaskKey::Startup {
        package: "pkg".into(),
        version: "1".into(),
        generation: 1,
    };
    let (start_rx, cancel_rx) = registry.reserve(key.clone()).expect("reservation");
    let (release_tx, release_rx) = oneshot::channel();
    let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let completed_task = completed.clone();
    let task = tokio::spawn(async move {
        assert!(start_rx.await.expect("start gate"));
        let _ = cancel_rx.await;
        let _ = release_rx.await;
        completed_task.store(true, Ordering::SeqCst);
    });
    registry.install(&key, task).expect("install");

    assert!(
        !registry
            .cancel_matching(|candidate| candidate == &key)
            .await
    );
    assert!(!completed.load(Ordering::SeqCst));
    assert!(registry.contains_quarantined(&key));
    assert_eq!(registry.len(), 1);
    assert!(registry
        .reserve(TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation: 2,
        })
        .is_none());
    assert!(
        !registry
            .cancel_matching(|candidate| candidate == &key)
            .await
    );
    assert!(!registry.shutdown().await);
    assert!(registry.contains_quarantined(&key));

    release_tx.send(()).expect("release startup");
    tokio::task::yield_now().await;
    registry.reap_completed().await;
    assert!(completed.load(Ordering::SeqCst));
    assert_eq!(registry.len(), 0);
    assert!(registry.shutdown().await);
}
