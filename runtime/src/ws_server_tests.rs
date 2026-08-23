// ----- Unit tests: hello validation без network -----
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_message_limit_accepts_five_minute_dictation_wav() {
        // Regression: 2026-08-19. Base64 WAV used to exceed the 1 MiB WS limit
        // and tungstenite closed the whole Engine connection before dispatch.
        const PCM_BYTES_PER_SECOND: usize = 16_000 * 2;
        const FIVE_MINUTE_WAV_BASE64_BYTES: usize = ((44 + PCM_BYTES_PER_SECOND * 300) + 2) / 3 * 4;
        assert!(MAX_WS_MESSAGE_BYTES >= FIVE_MINUTE_WAV_BASE64_BYTES + 1024);
    }
    use crate::app_index::{App, AppKind};
    use tokio::io::AsyncReadExt;

    fn baseline_hello() -> HelloMessage {
        HelloMessage {
            kind: Some("hello".into()),
            protocol_version: None,
            api_version: Some(API_VERSION.into()),
            token: Some("test-token".into()),
            pid: Some(std::process::id()),
            client_id: Some("eden".into()),
            client_class: Some("@kosmos/ark".into()),
            client_version: Some("0.1.0".into()),
        }
    }

    fn accepted_compat(outcome: HelloOutcome) -> Compatibility {
        let HelloOutcome::Accept { compatibility, .. } = outcome else {
            assert!(
                matches!(outcome, HelloOutcome::Accept { .. }),
                "expected accept"
            );
            unreachable!();
        };
        compatibility
    }

    fn rejected_code(outcome: HelloOutcome) -> &'static str {
        let HelloOutcome::Reject { code, .. } = outcome else {
            assert!(
                matches!(outcome, HelloOutcome::Reject { .. }),
                "expected reject"
            );
            unreachable!();
        };
        code
    }

    #[test]
    fn legacy_protocol_version_requires_upgrade() {
        let mut hello = baseline_hello();
        hello.api_version = None;
        hello.protocol_version = Some("1.0.0".into());
        let outcome = validate_hello(&hello, "test-token");
        assert_eq!(rejected_code(outcome), handshake_errors::UPGRADE_REQUIRED);
    }

    #[test]
    fn valid_api_v1_hello_accepted() {
        let hello = baseline_hello();
        let outcome = validate_hello(&hello, "test-token");
        assert!(matches!(
            outcome,
            HelloOutcome::Accept {
                compatibility: Compatibility::Exact,
                transport: TransportKind::ApiV1
            }
        ));
    }

    #[test]
    fn api_v1_missing_hello_kind_is_rejected() {
        let mut hello = baseline_hello();
        hello.kind = None;
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::MALFORMED_HELLO
        );
    }

    #[test]
    fn legacy_missing_hello_kind_requires_upgrade() {
        let mut hello = baseline_hello();
        hello.api_version = None;
        hello.protocol_version = Some("1.0.0".into());
        hello.kind = None;
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::UPGRADE_REQUIRED
        );
    }

    #[test]
    fn hello_with_both_versions_is_rejected() {
        let mut hello = baseline_hello();
        hello.protocol_version = Some("1.0.0".into());
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::UPGRADE_REQUIRED
        );
    }

    #[test]
    fn missing_protocol_version_rejected() {
        let mut hello = baseline_hello();
        hello.api_version = None;
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::MISSING_PROTOCOL_VERSION
        );
    }

    #[test]
    fn malformed_protocol_version_rejected() {
        let mut hello = baseline_hello();
        hello.api_version = Some("not-a-version".into());
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::MALFORMED_PROTOCOL_VERSION
        );
    }

    #[test]
    fn major_mismatch_rejected_as_incompatible() {
        let mut hello = baseline_hello();
        hello.api_version = Some("2.0.0".into());
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::INCOMPATIBLE_PROTOCOL_VERSION
        );
    }

    #[test]
    fn minor_mismatch_accepted() {
        let mut hello = baseline_hello();
        hello.api_version = Some("1.99.0".into());
        assert_eq!(
            accepted_compat(validate_hello(&hello, "test-token")),
            Compatibility::MinorMismatch
        );
    }

    #[test]
    fn missing_token_rejected() {
        let mut hello = baseline_hello();
        hello.token = None;
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::MISSING_TOKEN
        );
    }

    #[test]
    fn invalid_token_rejected() {
        let mut hello = baseline_hello();
        hello.token = Some("wrong-token".into());
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::INVALID_TOKEN
        );
    }

    #[test]
    fn missing_pid_rejected() {
        let mut hello = baseline_hello();
        hello.pid = None;
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::MISSING_PID
        );
    }

    #[test]
    fn nonexistent_pid_rejected() {
        let mut hello = baseline_hello();
        hello.pid = Some(0x7FFFFFFF); // impossibly high
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::INVALID_PID
        );
    }

    #[test]
    fn compatibility_label_strings() {
        assert_eq!(compatibility_label(&Compatibility::Exact), "exact");
        assert_eq!(
            compatibility_label(&Compatibility::MinorMismatch),
            "minor_mismatch"
        );
        assert_eq!(
            compatibility_label(&Compatibility::Incompatible),
            "incompatible"
        );
    }

    #[test]
    fn app_index_entry_uses_icon_ref_without_inline_data_url() {
        // Regression: 2026-06-09. app_index.search must not read/base64 top-N icons.
        let app = App {
            id: "calc".into(),
            name: "Calculator".into(),
            exec_path: "C:\\Windows\\System32\\calc.exe".into(),
            icon_path: Some("C:\\Kosmos\\icons\\calc.png".into()),
            icon_source: None,
            kind: AppKind::Win32,
            source: "test".into(),
            mtime: 1,
        };

        let entry = app_index_entry_json(&app);

        assert_eq!(entry["icon_path"], serde_json::Value::Null);
        assert_eq!(entry["icon_ref"], "kosmos-icon://app/calc");
    }

    #[tokio::test]
    async fn calculator_op_returns_result_or_quiet_null() {
        let data_dir = tempfile::tempdir().unwrap();
        let result = handle_calculator_op(
            "evaluate",
            serde_json::json!({ "query": "1200 * 1.2" }),
            data_dir.path(),
        )
        .await;
        assert!(result.ok);
        assert_eq!(result.data["result"], "1440");
        assert_eq!(result.data["expression"], "1200 * 1.2");

        let search_text = handle_calculator_op(
            "evaluate",
            serde_json::json!({ "query": "settings" }),
            data_dir.path(),
        )
        .await;
        assert!(search_text.ok);
        assert!(search_text.data["result"].is_null());
    }

    #[tokio::test]
    async fn file_index_diagnostics_op_returns_enriched_payload() {
        let data = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("diag-note.md"), "v1").unwrap();
        let index =
            Arc::new(FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap());
        index.rescan().await.unwrap();

        let response = handle_file_index_op("diagnostics", serde_json::Value::Null, &index).await;

        assert!(response.ok);
        assert_eq!(response.data["roots_count"], 1);
        assert_eq!(response.data["files_count"], 1);
        assert!(response.data.get("db_size_bytes").is_some());
    }

    #[tokio::test]
    async fn file_index_estimate_root_op_returns_estimate_payload() {
        let data = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("notes.md"), "v1").unwrap();
        std::fs::write(root.path().join("photo.png"), "v1").unwrap();
        let index =
            Arc::new(FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap());

        let response = handle_file_index_op(
            "estimate_root",
            serde_json::json!({ "path": root.path().to_string_lossy() }),
            &index,
        )
        .await;

        assert!(response.ok);
        assert_eq!(response.data["indexable_text_files_count"], 1);
        assert_eq!(response.data["metadata_only_media_files_count"], 1);
    }

    #[tokio::test]
    async fn package_api_returns_bounded_metadata_without_trust_material_or_paths() {
        let data = tempfile::tempdir().unwrap();
        let service = Arc::new(PackageService::open(data.path()).unwrap());

        let status = handle_package_op("trust_status", serde_json::Value::Null, &service).await;
        assert!(status.ok);
        let status_json = status.data.to_string();
        for forbidden in [
            "public_key",
            "signature",
            "archive_path",
            "entrypoint",
            "permissions",
            "sha256",
        ] {
            assert!(!status_json.contains(forbidden), "{status_json}");
        }

        let list = handle_package_op("list", serde_json::Value::Null, &service).await;
        assert!(list.ok);
        assert_eq!(list.data["total"], 0);
        assert_eq!(list.data["truncated"], false);

        let private_path = r"C:\Users\alice\private\package.kspkg";
        let install = handle_package_op(
            "install",
            serde_json::json!({
                "id": "com.kosmos.demo",
                "version": "1.0.0",
                "archive_path": private_path,
            }),
            &service,
        )
        .await;
        assert!(!install.ok);
        assert!(!install.error.unwrap_or_default().contains(private_path));
    }

    async fn production_ws_fixture() -> (tempfile::TempDir, WsServer) {
        let dir = tempfile::tempdir().unwrap();
        let binary = crate::ark_host::resolve_ark_core_rpc_path()
            .expect("real ark-core-rpc fixture must be built");
        let ark = Arc::new(
            crate::ark_host::ArkHost::spawn(&binary, &dir.path().join("ark.db").to_string_lossy())
                .await
                .unwrap(),
        );
        let ws = WsServer::bind(
            ark,
            "test-token".repeat(8),
            dir.path().to_path_buf(),
            Arc::new(
                crate::app_index::AppIndex::new(dir.path(), dir.path().join("icons")).unwrap(),
            ),
            Arc::new(crate::file_index::FileIndex::new_disabled(dir.path()).unwrap()),
            Arc::new(crate::usage_tracker::UsageTrackerDiagnosticsState::default()),
            Arc::new(crate::protocol_usage::ProtocolUsageStore::open(dir.path()).unwrap()),
            Arc::new(crate::package_service::PackageService::open(dir.path()).unwrap()),
            "00000000-0000-4000-8000-000000000001".into(),
        )
        .await
        .unwrap();
        (dir, ws)
    }

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

    #[tokio::test]
    async fn production_ws_shutdown_reaps_authenticated_and_stalled_lifecycles() {
        let (_dir, server) = production_ws_fixture().await;
        let shutdown = server.shutdown_handle();
        shutdown.set_response_deadline(Duration::from_millis(50));
        let port = server.port();
        let dispatcher = server.dispatcher();
        let bus = server.command_bus_handle();
        let task = tokio::spawn(server.run());

        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .unwrap();
        socket
            .send(Message::Text(
                serde_json::json!({
                    "kind": "hello",
                    "apiVersion": API_VERSION,
                    "token": "test-token".repeat(8),
                    "pid": std::process::id(),
                })
                .to_string(),
            ))
            .await
            .unwrap();
        assert!(
            socket
                .next()
                .await
                .unwrap()
                .unwrap()
                .to_text()
                .unwrap()
                .contains("hello_ok")
        );
        socket
            .send(Message::Text(
                r#"{"operation":"commands.register","commands":[{"id":"lifecycle.command","title":"Lifecycle","category":"test"}]}"#.into(),
            ))
            .await
            .unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(1), socket.next())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while bus.registration_count_sync() != 1 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();

        let mut raw = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        let (mut no_hello, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .unwrap();
        let _ = (&mut raw, &mut no_hello);
        tokio::time::timeout(Duration::from_secs(1), async {
            while shutdown.task_count() < 3 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();

        shutdown.begin_shutdown().await;
        task.await.unwrap().unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(1), socket.next())
                .await
                .is_ok()
        );
        let first_shutdown = shutdown.shutdown().await;
        assert!(
            first_shutdown.is_err(),
            "stalled lifecycle must report deadline breach"
        );
        shutdown.shutdown().await.unwrap();
        assert_eq!(shutdown.task_count(), 0);
        assert_eq!(dispatcher.live_owner_count(), 0);
        assert_eq!(bus.registration_count_sync(), 0);
    }

    #[tokio::test]
    async fn production_ws_capacity_rejects_the_next_raw_socket_and_restores_capacity() {
        let (_dir, server) = production_ws_fixture().await;
        let port = server.port();
        let dispatcher = server.dispatcher();
        let shutdown = server.shutdown_handle();
        let task = tokio::spawn(server.run());
        let mut sockets = Vec::with_capacity(MAX_ACTIVE_WS_CONNECTIONS + 1);
        for _ in 0..=MAX_ACTIVE_WS_CONNECTIONS {
            sockets.push(
                tokio::net::TcpStream::connect(("127.0.0.1", port))
                    .await
                    .unwrap(),
            );
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while shutdown.task_count() != MAX_ACTIVE_WS_CONNECTIONS {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let mut rejected = sockets.pop().unwrap();
        let mut byte = [0u8; 1];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), rejected.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        shutdown.shutdown().await.unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(shutdown.task_count(), 0);
        assert_eq!(dispatcher.live_owner_count(), 0);
        assert_eq!(shutdown.available_capacity(), MAX_ACTIVE_WS_CONNECTIONS);
    }

    async fn authenticated_socket(
        port: u16,
        token: &str,
    ) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>
    {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .unwrap();
        socket
            .send(Message::Text(
                serde_json::json!({
                    "kind": "hello",
                    "apiVersion": API_VERSION,
                    "token": token,
                    "pid": std::process::id(),
                })
                .to_string(),
            ))
            .await
            .unwrap();
        assert!(
            socket
                .next()
                .await
                .unwrap()
                .unwrap()
                .to_text()
                .unwrap()
                .contains("hello_ok")
        );
        socket
    }

    #[tokio::test]
    async fn production_ws_reaps_each_sequential_request_on_one_connection() {
        let (_dir, server) = production_ws_fixture().await;
        let shutdown = server.shutdown_handle();
        let dispatcher = server.dispatcher();
        let bus = server.command_bus_handle();
        let port = server.port();
        let task = tokio::spawn(server.run());
        let mut socket = authenticated_socket(port, &"test-token".repeat(8)).await;

        for id in 0..10_000_u32 {
            socket
                .send(Message::Text(
                    serde_json::json!({
                        "id": id.to_string(),
                        "operation": "commands.unregister",
                        "ids": [],
                    })
                    .to_string(),
                ))
                .await
                .unwrap();
            loop {
                let response = socket.next().await.unwrap().unwrap();
                let value: serde_json::Value =
                    serde_json::from_str(response.to_text().unwrap()).unwrap();
                if value.get("id") == Some(&serde_json::Value::String(id.to_string())) {
                    assert_eq!(value["ok"], true);
                    break;
                }
            }
            assert_eq!(shutdown.request_task_count(), 0);
        }

        assert_eq!(shutdown.request_task_count(), 0);
        assert_eq!(
            shutdown.lifecycle.request_capacity.available_permits(),
            MAX_ACTIVE_WS_REQUESTS
        );
        drop(socket);
        shutdown.shutdown().await.unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(shutdown.task_count(), 0);
        assert_eq!(shutdown.request_task_count(), 0);
        assert_eq!(dispatcher.live_owner_count(), 0);
        assert_eq!(bus.registration_count_sync(), 0);
    }

    #[tokio::test]
    async fn production_ws_disconnect_cancels_stalled_request_without_replay() {
        let (_dir, server) = production_ws_fixture().await;
        let shutdown = server.shutdown_handle();
        let dispatcher = server.dispatcher();
        let bus = server.command_bus_handle();
        let entered = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let entered_observer = entered.clone();
        dispatcher.set_test_observer(Some(Arc::new(move |phase, _, operation| {
            if matches!(phase, crate::engine_dispatch::DispatchPhase::Started)
                && operation == "test.stall"
            {
                entered_observer.fetch_add(1, Ordering::SeqCst);
            }
        })));
        let port = server.port();
        let task = tokio::spawn(server.run());
        let mut socket = authenticated_socket(port, &"test-token".repeat(8)).await;
        socket
            .send(Message::Text(
                serde_json::json!({
                    "id": "stalled",
                    "operation": "test.stall",
                })
                .to_string(),
            ))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while shutdown.request_task_count() != 1 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        socket
            .send(Message::Text(
                serde_json::json!({
                    "id": "second",
                    "operation": "test.stall",
                })
                .to_string(),
            ))
            .await
            .unwrap();
        let busy = tokio::time::timeout(Duration::from_secs(1), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let busy: serde_json::Value = serde_json::from_str(busy.to_text().unwrap()).unwrap();
        assert_eq!(busy["ok"], false);
        assert_eq!(busy["error"], "WS request busy");
        tokio::time::timeout(Duration::from_secs(1), async {
            while entered.load(Ordering::SeqCst) != 1 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        drop(socket);
        tokio::time::timeout(Duration::from_secs(1), async {
            while shutdown.request_task_count() != 0 || dispatcher.live_owner_count() != 0 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(entered.load(Ordering::SeqCst), 1);
        shutdown.shutdown().await.unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(shutdown.request_task_count(), 0);
        assert_eq!(dispatcher.live_owner_count(), 0);
        assert_eq!(bus.registration_count_sync(), 0);
    }
}
