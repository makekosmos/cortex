    use super::*;
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

    #[tokio::test]
    async fn task_registry_rejects_duplicate_and_capacity_overflow() {
        let registry = TaskRegistry::new(1);
        let first = TaskKey::Lifecycle {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        };
        let second = TaskKey::Lifecycle {
            package: "pkg".into(),
            version: "1".into(),
            generation: 2,
        };
        assert!(registry.reserve(first.clone()).is_some());
        assert!(registry.reserve(first).is_none());
        assert!(registry.reserve(second).is_none());
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn startup_key_is_generation_specific_and_registry_owned() {
        let registry = TaskRegistry::new(2);
        let key = TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation: 9,
        };
        let (start_rx, cancel_rx) = registry.reserve(key.clone()).expect("startup reservation");
        let task = tokio::spawn(async move {
            assert!(start_rx.await.expect("startup gate"));
            let _ = cancel_rx.await;
        });
        registry.install(&key, task).expect("startup install");
        assert!(registry.contains(&key));
        assert!(registry.cancel_generation("pkg", "1", 9).await);
        assert!(!registry.contains(&key));
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn new_generation_has_an_exact_distinct_lifecycle_key() {
        let registry = TaskRegistry::new(2);
        let old = TaskKey::Lifecycle {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        };
        let new = TaskKey::Lifecycle {
            package: "pkg".into(),
            version: "1".into(),
            generation: 2,
        };
        let (_old_start, _old_cancel) = registry.reserve(old).expect("old lifecycle");
        let (_new_start, _new_cancel) = registry.reserve(new).expect("new lifecycle");
        assert_eq!(registry.len(), 2);
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn generation_cleanup_is_exact_and_preserves_replacement_slots() {
        let registry = TaskRegistry::new(6);
        let old_keys = [
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
        let replacement = TaskKey::WorkerStdin {
            package: "pkg".into(),
            version: "1".into(),
            generation: 2,
        };
        for key in old_keys.iter().chain(std::iter::once(&replacement)) {
            let (start, _cancel) = registry.reserve(key.clone()).expect("reservation");
            drop(start);
        }
        assert!(registry.has_generation("pkg", "1", 1));
        assert!(registry.has_generation("pkg", "1", 2));
        assert!(registry.cancel_generation("pkg", "1", 1).await);
        assert!(!registry.has_generation("pkg", "1", 1));
        assert!(registry.has_generation("pkg", "1", 2));
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn stale_finish_cannot_claim_replacement_generation() {
        let supervisor = PackageWorkerSupervisor::new(1);
        let key = ("pkg".into(), "1".into());
        supervisor.insert_failed(key.clone());
        {
            let mut workers = lock(&supervisor.inner.workers);
            let worker = workers.get_mut(&key).expect("worker");
            worker.generation = 2;
            worker.lifecycle_reason = Some("replacement".into());
        }
        finish_inner_until(
            &supervisor.inner,
            &key,
            1,
            WorkerState::Failed,
            Instant::now() + STOP_DEADLINE,
        )
        .await;
        let workers = lock(&supervisor.inner.workers);
        let worker = workers.get(&key).expect("replacement");
        assert_eq!(worker.generation, 2);
        assert_eq!(worker.lifecycle_reason.as_deref(), Some("replacement"));
    }
    #[test]
    fn cleanup_failure_is_a_distinct_diagnostic_reason() {
        let supervisor = PackageWorkerSupervisor::new(1);
        let key = ("cleanup".into(), "1".into());
        supervisor.insert_failed(key.clone());
        if let Some(worker) = lock(&supervisor.inner.workers).get_mut(&key) {
            worker.lifecycle_reason = Some("cleanup-failed".into());
        }
        assert_eq!(
            supervisor.diagnostics()[0].lifecycle_reason.as_deref(),
            Some("cleanup-failed")
        );
    }
    #[test]
    fn app_and_non_exe_fail_closed() {
        let mut m = crate::package_manifest::PackageManifest {
            schema_version: 1,
            id: "x".into(),
            name: "x".into(),
            version: "1.0.0".into(),
            kind: PackageKind::App,
            engine_api: ">=1".into(),
            entrypoint: "x.exe".into(),
            publisher: "kosmos".into(),
            permissions: vec![],
        };
        assert!(PackageWorkerSupervisor::validate_manifest(&m).is_err());
        m.kind = PackageKind::Source;
        m.entrypoint = "x.js".into();
        assert!(PackageWorkerSupervisor::validate_manifest(&m).is_err());
    }
    #[test]
    fn token_hash_does_not_accept_wrong_hello_token() {
        assert!(!token_matches(&hash_token("right"), "wrong"));
    }

    #[test]
    fn restart_policy_uses_bounded_backoff() {
        assert_eq!(restart_delay(1), Some(Duration::from_secs(1)));
        assert_eq!(restart_delay(2), Some(Duration::from_secs(5)));
        assert_eq!(restart_delay(3), Some(Duration::from_secs(30)));
        assert_eq!(restart_delay(4), None);
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn start_is_unsupported_on_non_windows_and_records_failure() {
        let supervisor = PackageWorkerSupervisor::new(1);
        let manifest = PackageManifest {
            schema_version: 1,
            id: "linux-test".into(),
            name: "linux-test".into(),
            version: "1.0.0".into(),
            kind: PackageKind::Source,
            engine_api: ">=1".into(),
            entrypoint: "worker.exe".into(),
            publisher: "kosmos".into(),
            permissions: vec![],
        };
        let result = supervisor
            .start(
                &manifest,
                PathBuf::from("/tmp/worker.exe"),
                "hash".into(),
                &[],
                "correlation".into(),
                None,
            )
            .await;
        assert_eq!(result, Err("unsupported-platform"));
        assert_eq!(
            supervisor.health("linux-test", "1.0.0").state,
            WorkerState::Failed
        );
    }

    #[test]
    fn diagnostics_are_sorted_bounded_and_redacted() {
        let supervisor = PackageWorkerSupervisor::new(1);
        supervisor.insert_failed(("b".into(), "1".into()));
        supervisor.insert_failed(("a".into(), "1".into()));
        let key = ("a".to_string(), "1".to_string());
        let workers = lock(&supervisor.inner.workers);
        let worker = workers.get(&key).expect("worker");
        lock(&worker.stdout_tail).push("token=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa alice@example.com C:\\Users\\alice\\note.txt");
        lock(&worker.stderr_tail).push("scope=filesystem.read /home/alice/private");
        drop(workers);
        let diagnostics = supervisor.diagnostics();
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].id, "a");
        let json = serde_json::to_string(&diagnostics).expect("json");
        for forbidden in [
            "secret",
            "alice@example.com",
            "note.txt",
            "filesystem.read",
            "/home/alice",
        ] {
            assert!(!json.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn bridge_status_is_bounded_and_path_free() {
        let status = sanitize_bridge_status(BridgeStatus {
            last_sync: Some("C:\\vault\\body".into()),
            conflict_count: u32::MAX,
            last_conflict_at: Some("2026-07-29T12:00:00Z".into()),
        });
        assert_eq!(status.last_sync, None);
        assert_eq!(status.conflict_count, 1_000_000);
        assert_eq!(
            status.last_conflict_at.as_deref(),
            Some("2026-07-29T12:00:00Z")
        );
    }
