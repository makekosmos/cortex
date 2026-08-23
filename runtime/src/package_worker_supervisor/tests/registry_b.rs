use super::super::*;

#[tokio::test]
async fn quarantined_startup_rejects_exact_replacement_and_preserves_original_handle() {
    let registry = TaskRegistry::owned(2);
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
    assert!(registry.contains_quarantined(&key));
    assert!(registry.reserve(key.clone()).is_none());
    assert!(registry
        .reserve(TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation: 2,
        })
        .is_some());

    assert!(
        !registry
            .cancel_matching(|candidate| candidate == &key)
            .await
    );
    assert!(registry.contains_quarantined(&key));
    assert!(!completed.load(Ordering::SeqCst));

    release_tx.send(()).expect("release startup");
    tokio::task::yield_now().await;
    registry.reap_completed().await;
    assert!(completed.load(Ordering::SeqCst));
    assert!(!registry.contains_quarantined(&key));
    assert!(registry.reserve(key.clone()).is_some());
    registry.remove(&key);
    registry.remove(&TaskKey::Startup {
        package: "pkg".into(),
        version: "1".into(),
        generation: 2,
    });
    assert!(registry.shutdown().await);
}

#[tokio::test]
async fn absent_worker_stop_reports_live_matching_startup_quarantine() {
    let supervisor = PackageWorkerSupervisor::new(1);
    let key = TaskKey::Startup {
        package: "pkg".into(),
        version: "1".into(),
        generation: 4,
    };
    let (start_rx, cancel_rx) = supervisor
        .inner
        .startups
        .reserve(key.clone())
        .expect("startup reservation");
    let (release_tx, release_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        assert!(start_rx.await.expect("start gate"));
        let _ = cancel_rx.await;
        let _ = release_rx.await;
    });
    supervisor
        .inner
        .startups
        .install(&key, task)
        .expect("startup install");
    assert!(
        !supervisor
            .inner
            .startups
            .cancel_matching(|candidate| candidate == &key)
            .await
    );

    assert_eq!(supervisor.stop("pkg", "1").await, Err("cleanup-failed"));

    release_tx.send(()).expect("release startup");
    tokio::task::yield_now().await;
    supervisor.inner.startups.reap_completed().await;
    assert_eq!(supervisor.stop("pkg", "1").await, Ok(()));
    assert!(supervisor.stop_all().await.is_ok());
}

#[tokio::test]
async fn absent_worker_stop_is_ok_after_completed_startup_is_reaped() {
    let supervisor = PackageWorkerSupervisor::new(1);
    let key = TaskKey::Startup {
        package: "pkg".into(),
        version: "1".into(),
        generation: 5,
    };
    let (start_rx, _cancel_rx) = supervisor
        .inner
        .startups
        .reserve(key.clone())
        .expect("startup reservation");
    let task = tokio::spawn(async move {
        assert!(start_rx.await.expect("start gate"));
    });
    supervisor
        .inner
        .startups
        .install_pending(&key, task)
        .expect("startup install");
    assert!(supervisor.inner.startups.open(&key));
    tokio::task::yield_now().await;
    assert_eq!(supervisor.stop("pkg", "1").await, Ok(()));
    assert_eq!(supervisor.inner.startups.len(), 0);
}

#[tokio::test]
async fn join_key_reports_a_running_quarantined_task_without_dropping_it() {
    let registry = TaskRegistry::owned(1);
    let key = TaskKey::Startup {
        package: "pkg".into(),
        version: "1".into(),
        generation: 1,
    };
    let (start_rx, cancel_rx) = registry.reserve(key.clone()).expect("reservation");
    let (release_tx, release_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        assert!(start_rx.await.expect("start gate"));
        let _ = cancel_rx.await;
        let _ = release_rx.await;
    });
    registry.install(&key, task).expect("install");
    assert!(
        !registry
            .cancel_matching(|candidate| candidate == &key)
            .await
    );

    assert!(!registry.join_key(&key).await);
    assert!(registry.contains_quarantined(&key));

    release_tx.send(()).expect("release startup");
    tokio::task::yield_now().await;
    assert!(registry.join_key(&key).await);
    assert_eq!(registry.len(), 0);
    assert!(registry.shutdown().await);
}

#[tokio::test]
async fn quarantine_insert_is_non_overwriting() {
    let key = TaskKey::Startup {
        package: "pkg".into(),
        version: "1".into(),
        generation: 1,
    };
    let (old_start, _old_cancel) = oneshot::channel();
    let (new_start, _new_cancel) = oneshot::channel();
    let old = TaskSlot {
        start: Some(old_start),
        cancel: None,
        task: Some(tokio::spawn(async {})),
        #[cfg(windows)]
        owner: None,
        #[cfg(windows)]
        process_holder: None,
    };
    let incoming = TaskSlot {
        start: Some(new_start),
        cancel: None,
        task: Some(tokio::spawn(async {})),
        #[cfg(windows)]
        owner: None,
        #[cfg(windows)]
        process_holder: None,
    };
    let mut quarantine = HashMap::new();
    assert!(TaskRegistry::insert_quarantine(&mut quarantine, key.clone(), old).is_ok());
    let incoming = TaskRegistry::insert_quarantine(&mut quarantine, key.clone(), incoming)
        .expect_err("duplicate quarantine key must not overwrite");
    assert!(quarantine.contains_key(&key));
    let incoming_task = incoming.task.expect("incoming handle preserved");
    incoming_task.await.expect("incoming task joined");
}

#[tokio::test]
async fn startup_completion_is_explicitly_reaped_for_large_batches() {
    let registry = TaskRegistry::owned(10_000);
    for generation in 0..10_000 {
        let key = TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation,
        };
        let (start_rx, _cancel_rx) = registry.reserve(key.clone()).expect("reservation");
        let task = tokio::spawn(async move {
            assert!(start_rx.await.expect("start gate"));
        });
        registry.install(&key, task).expect("install");
    }
    assert_eq!(registry.len(), 10_000);
    while registry.len() != 0 {
        tokio::task::yield_now().await;
        registry.reap_completed().await;
    }
    assert_eq!(registry.len(), 0);
}
