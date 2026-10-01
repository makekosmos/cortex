    // KOS-299 launch bridge: `packages.open` mints a per-tab lease, the
    // served page bootstraps its credentials from the URL fragment, and the
    // launch token — never the Engine bearer — authorizes the data channel.
    struct OpenFixture {
        // The TempDir must outlive the spawned server — stores keep open
        // handles inside it (KOS-270).
        _dir: tempfile::TempDir,
        port: u16,
        shutdown: EngineApiShutdownHandle,
        task: tokio::task::JoinHandle<Result<(), EngineApiError>>,
        leases: Arc<Mutex<LaunchLeaseRegistry>>,
        ark: Arc<crate::ark_host::ArkHost>,
    }

    impl OpenFixture {
        /// Tear-down order matters on Windows: the event-bus ArkHost keeps
        /// `ark.db` open, so it must drop before the TempDir sweeps the dir
        /// (KOS-270 gate-tmp rejects leftover fixture files).
        async fn shutdown(self) {
            let OpenFixture {
                _dir: dir,
                shutdown,
                task,
                leases,
                ark,
                ..
            } = self;
            let _ = shutdown.shutdown().await;
            let _ = task.await;
            drop(ark);
            drop(leases);
            drop(dir);
        }
    }

    async fn open_engine(
        dir: tempfile::TempDir,
        token: &str,
        service: Arc<PackageService>,
    ) -> OpenFixture {
        let dir_path = dir.path().to_path_buf();
        // A WsServer bound on the same temp dir owns the real dispatch table —
        // `packages.open` must run through it, not a stubbed dispatcher.
        let ark = Arc::new(
            crate::ark_host::ArkHost::open(&dir_path.join("ark.db").to_string_lossy())
                .await
                .expect("ark"),
        );
        let ws = crate::ws_server::WsServer::bind(
            ark.clone(),
            token.to_string(),
            dir_path.clone(),
            Arc::new(
                crate::app_index::AppIndex::new(&dir_path, dir_path.join("icons")).expect("app index"),
            ),
            Arc::new(crate::file_index::FileIndex::new_disabled(&dir_path).expect("file index")),
            Arc::new(crate::usage_tracker::UsageTrackerDiagnosticsState::default()),
            Arc::new(ProtocolUsageStore::open(&dir_path).expect("usage")),
            service.clone(),
            "00000000-0000-4000-8000-000000000001".into(),
        )
        .await
        .expect("ws server");
        let package_service = service;
        let mut server = EngineApiServer::bind(
            token.to_string(),
            ws.port(),
            Arc::new(ProtocolUsageStore::open(&dir_path.join("usage2")).expect("usage2")),
            "00000000-0000-4000-8000-000000000001".into(),
            package_service,
            Arc::new(ws.dispatcher()),
        )
        .await
        .expect("server");
        server.set_launch_events(ark.subscribe_events());
        let port = server.port();
        let shutdown = server.shutdown_handle();
        let leases = Arc::clone(&server.launch_leases);
        let task = tokio::spawn(server.run());
        OpenFixture {
            _dir: dir,
            port,
            shutdown,
            task,
            leases,
            ark,
        }
    }

    fn launch_bootstrap_parts(launch_url: &str, port: u16) -> (String, String, String) {
        let (path, fragment) = launch_url
            .split_once(&format!(":{port}"))
            .expect("loopback launch_url")
            .1
            .split_once('#')
            .expect("launch_url carries the bootstrap fragment");
        let mut launch_id = None;
        let mut code = None;
        for pair in fragment.split('&') {
            if let Some(value) = pair.strip_prefix("launch=") {
                launch_id = Some(value.to_string());
            } else if let Some(value) = pair.strip_prefix("code=") {
                code = Some(value.to_string());
            }
        }
        (
            path.to_string(),
            launch_id.expect("fragment launch id"),
            code.expect("fragment bootstrap code"),
        )
    }

    /// The per-package origin the page is served from — each package gets
    /// its own localStorage/IndexedDB namespace, so sibling packages can
    /// never share one.
    fn package_origin(port: u16) -> String {
        format!(
            "http://{}:{port}",
            crate::package_launch::package_origin_host("com.kosmos.demo")
        )
    }

    async fn rpc_open(port: u16, token: &str, package_id: &str) -> Value {
        response_json(
            &raw_http(
                port,
                &request(
                    token,
                    "POST",
                    "/v1/rpc",
                    &format!(
                        r#"{{"operation":"packages.open","package_id":"{package_id}"}}"#
                    ),
                ),
            )
            .await,
        )
    }

    async fn bootstrap(port: u16, launch_id: &str, code: &str, origin: &str) -> String {
        let body = format!(r#"{{"code":"{code}"}}"#);
        raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{launch_id}/bootstrap HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\n\
                 Origin: {origin}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            ),
        )
        .await
    }

    fn ark_request(port: u16, launch_id: &str, launch_token: &str, operation: &str) -> String {
        let body = format!(r#"{{"operation":"{operation}","params":{{}}}}"#);
        format!(
            "POST /v1/apps/launch/{launch_id}/ark HTTP/1.1\r\n\
             Host: 127.0.0.1:{port}\r\n\
             X-Kosmos-Launch-Token: {launch_token}\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        )
    }

    /// End-to-end in isolation: `packages.open` on the dispatch channel mints
    /// a fresh lease, the launch_url fragment hands the page a one-time code,
    /// and the bootstrap exchange turns it into a working launch token —
    /// all without the Engine bearer ever leaving the Manager.
    #[tokio::test]
    async fn packages_open_serves_a_bootstrapped_data_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let service = Arc::new(
            crate::package_service::tests::enabled_note_write_app_service(dir.path()),
        );
        let fixture = open_engine(dir, &token, service).await;
        let port = fixture.port;

        let opened = rpc_open(port, &token, "com.kosmos.demo").await;
        assert_eq!(opened["ok"], true, "{opened}");
        let data = &opened["data"];
        // Only what the Manager uses — the broker token must NOT cross to it.
        assert!(data["broker_token"].is_null(), "{data}");
        assert!(data["data_api"].is_null(), "{data}");
        assert!(data["asset_token"].is_null(), "{data}");
        let launch_url = data["launch_url"].as_str().expect("launch_url");
        // The page is served on the package's own *.localhost origin — not
        // the shared 127.0.0.1 origin every sibling would see.
        assert!(launch_url.starts_with(&package_origin(port)), "{launch_url}");
        assert!(!launch_url.contains("127.0.0.1"), "{launch_url}");
        let (asset_path, launch_id, code) = launch_bootstrap_parts(launch_url, port);
        assert!(!launch_url.contains("broker_token"), "{launch_url}");

        // The served entrypoint carries the injected host shim, and the shim
        // itself is served under the same asset token.
        let html = raw_http(
            port,
            &format!("GET {asset_path} HTTP/1.1\r\nConnection: close\r\n\r\n"),
        )
        .await;
        assert!(html.starts_with("HTTP/1.1 200"), "{html}");
        assert!(
            html.contains(r#"<script src="__kosmos_host_shim.js"></script>"#),
            "{html}"
        );
        assert!(!html.contains(&code), "the shim HTML must not embed the code");
        let shim = raw_http(
            port,
            &format!(
                "GET {}/__kosmos_host_shim.js HTTP/1.1\r\nConnection: close\r\n\r\n",
                asset_path.rsplit_once('/').expect("dir").0
            ),
        )
        .await;
        assert!(shim.starts_with("HTTP/1.1 200"), "{shim}");
        assert!(shim.contains("kosmosApp"), "{shim}");
        assert!(shim.contains("bootstrap"), "{shim}");

        // Bootstrap exchange: no bearer, and only this package's own
        // origin — neither the bare loopback host nor a sibling's origin
        // may spend the code.
        let wrong_origin = bootstrap(port, &launch_id, &code, "https://evil.example").await;
        assert!(wrong_origin.starts_with("HTTP/1.1 403"), "{wrong_origin}");
        let sibling_origin = bootstrap(
            port,
            &launch_id,
            &code,
            &format!(
                "http://{}:{port}",
                crate::package_launch::package_origin_host("com.kosmos.other")
            ),
        )
        .await;
        assert!(sibling_origin.starts_with("HTTP/1.1 403"), "{sibling_origin}");
        let bare_loopback = bootstrap(
            port,
            &launch_id,
            &code,
            &format!("http://127.0.0.1:{port}"),
        )
        .await;
        assert!(bare_loopback.starts_with("HTTP/1.1 403"), "{bare_loopback}");
        let exchanged = bootstrap(
            port,
            &launch_id,
            &code,
            &package_origin(port),
        )
        .await;
        assert!(exchanged.starts_with("HTTP/1.1 200"), "{exchanged}");
        let session = response_json(&exchanged);
        let broker_token = session["data"]["broker_token"].as_str().expect("token");
        assert_eq!(session["data"]["launch_id"].as_str(), Some(launch_id.as_str()));

        // Single-use: the same code must not be replayable.
        let replay = bootstrap(
            port,
            &launch_id,
            &code,
            &package_origin(port),
        )
        .await;
        assert!(replay.starts_with("HTTP/1.1 403"), "{replay}");
        assert!(replay.contains("bootstrap denied"), "{replay}");

        // The launch token — alone, no bearer — authorizes the data channel.
        // A granted read succeeds; a write outside the grant is refused.
        let allowed = raw_http(
            port,
            &ark_request(port, &launch_id, broker_token, "list_object_types"),
        )
        .await;
        assert!(allowed.starts_with("HTTP/1.1 200"), "{allowed}");
        assert_eq!(response_json(&allowed)["ok"], true, "{allowed}");
        let denied = raw_http(
            port,
            &ark_request(port, &launch_id, broker_token, "delete_object"),
        )
        .await;
        assert_eq!(response_json(&denied)["ok"], false, "{denied}");

        fixture.shutdown().await;
    }

    /// Two opens are two sessions: independent leases, codes and tokens —
    /// closing one tab must not kill the other (the old `already_running`
    /// reuse would have shared the token).
    #[tokio::test]
    async fn repeat_opens_are_independent_sessions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let service = Arc::new(
            crate::package_service::tests::enabled_note_write_app_service(dir.path()),
        );
        let fixture = open_engine(dir, &token, service).await;
        let port = fixture.port;

        let first = rpc_open(port, &token, "com.kosmos.demo").await;
        let second = rpc_open(port, &token, "com.kosmos.demo").await;
        assert_eq!(first["ok"], true);
        assert_eq!(second["ok"], true);
        let (_, first_id, first_code) =
            launch_bootstrap_parts(first["data"]["launch_url"].as_str().unwrap(), port);
        let (_, second_id, second_code) =
            launch_bootstrap_parts(second["data"]["launch_url"].as_str().unwrap(), port);
        assert_ne!(first_id, second_id);
        assert_ne!(first_code, second_code);

        // A code minted for one lease must not bootstrap the other.
        let crossed = bootstrap(
            port,
            &second_id,
            &first_code,
            &package_origin(port),
        )
        .await;
        assert!(crossed.starts_with("HTTP/1.1 403"), "{crossed}");

        let origin = package_origin(port);
        let first_session = response_json(&bootstrap(port, &first_id, &first_code, &origin).await);
        let second_session =
            response_json(&bootstrap(port, &second_id, &second_code, &origin).await);
        let first_token = first_session["data"]["broker_token"].as_str().unwrap();
        let second_token = second_session["data"]["broker_token"].as_str().unwrap();
        assert_ne!(first_token, second_token);

        // Pagehide-style revoke ends only its own session.
        let body = format!(r#"{{"token":"{first_token}"}}"#);
        let revoked = raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{first_id}/revoke HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            ),
        )
        .await;
        assert!(revoked.starts_with("HTTP/1.1 200"), "{revoked}");
        let dead = raw_http(
            port,
            &ark_request(port, &first_id, first_token, "list_object_types"),
        )
        .await;
        assert!(dead.starts_with("HTTP/1.1 403"), "{dead}");
        let live = raw_http(
            port,
            &ark_request(port, &second_id, second_token, "list_object_types"),
        )
        .await;
        assert_eq!(response_json(&live)["ok"], true, "{live}");

        // A wrong revoke token is denied, and a dead lease cannot bootstrap.
        let wrong_body = r#"{"token":"deadbeef"}"#;
        let wrong = raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{second_id}/revoke HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{wrong_body}",
                wrong_body.len()
            ),
        )
        .await;
        assert!(wrong.starts_with("HTTP/1.1 403"), "{wrong}");
        fixture.shutdown().await;
    }

    /// Wrong, expired and spent codes all collapse to one denial — the
    /// endpoint gives no oracle about which check failed.
    #[tokio::test]
    async fn bootstrap_denies_expired_and_wrong_codes_uniformly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let service = Arc::new(
            crate::package_service::tests::enabled_note_write_app_service(dir.path()),
        );
        let fixture = open_engine(dir, &token, service).await;
        let port = fixture.port;
        let origin = package_origin(port);

        let opened = rpc_open(port, &token, "com.kosmos.demo").await;
        let (_, launch_id, code) =
            launch_bootstrap_parts(opened["data"]["launch_url"].as_str().unwrap(), port);

        // Expired code → denied while the lease itself is still alive.
        fixture
            .leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .expire_bootstrap(&launch_id);
        let expired = bootstrap(port, &launch_id, &code, &origin).await;
        assert!(expired.starts_with("HTTP/1.1 403"), "{expired}");
        assert!(expired.contains("bootstrap denied"), "{expired}");

        // Wrong code → same denial, and a code-free request too.
        let wrong = bootstrap(port, &launch_id, &"0".repeat(64), &origin).await;
        assert_eq!(
            wrong.lines().next().unwrap(),
            expired.lines().next().unwrap(),
            "wrong vs expired codes must be indistinguishable"
        );
        let empty = raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{launch_id}/bootstrap HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\nOrigin: {origin}\r\n\
                 Content-Length: 2\r\nConnection: close\r\n\r\n{{}}"
            ),
        )
        .await;
        assert!(empty.starts_with("HTTP/1.1 403"), "{empty}");

        fixture.shutdown().await;
    }

    /// Typed failures stay typed: not-installed and disabled still report
    /// their wire codes, and the launch-scoped channel still refuses the
    /// Manager's `packages.open` itself.
    #[tokio::test]
    async fn packages_open_reports_typed_errors_and_stays_manager_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let service = crate::package_service::tests::enabled_note_write_app_service(dir.path());
        let fixture = open_engine(dir, &token, Arc::new(service)).await;
        let port = fixture.port;

        // The dispatch channel itself is still bearer-authenticated.
        let anon = raw_http(
            port,
            "POST /v1/rpc HTTP/1.1\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
        )
        .await;
        assert!(anon.starts_with("HTTP/1.1 401"), "{anon}");

        let missing = rpc_open(port, &token, "com.kosmos.missing").await;
        assert_eq!(missing["ok"], false);
        assert_eq!(missing["error"], "packages.open: not-installed");

        // The launch-scoped data channel refuses `packages.open` — the app
        // allowlist is not the Manager's op namespace.
        let opened = rpc_open(port, &token, "com.kosmos.demo").await;
        let (_, launch_id, code) =
            launch_bootstrap_parts(opened["data"]["launch_url"].as_str().unwrap(), port);
        let session = response_json(&bootstrap(
            port,
            &launch_id,
            &code,
            &package_origin(port),
        ).await);
        let launch_token = session["data"]["broker_token"].as_str().unwrap();
        let refused = raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{launch_id}/ark HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\n\
                 X-Kosmos-Launch-Token: {launch_token}\r\n\
                 Content-Length: 29\r\n\
                 Connection: close\r\n\r\n{{\"operation\":\"packages.open\"}}",
            ),
        )
        .await;
        assert!(refused.starts_with("HTTP/1.1 400"), "{refused}");

        fixture.shutdown().await;
    }

    /// Disabled packages report the typed code — exercised on the same
    /// open path.
    #[tokio::test]
    async fn packages_open_reports_disabled() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
        service
            .set_enabled("com.kosmos.demo", "1.0.0", false)
            .await
            .expect("disable");
        let fixture = open_engine(dir, &token, Arc::new(service)).await;
        let disabled = rpc_open(fixture.port, &token, "com.kosmos.demo").await;
        assert_eq!(disabled["ok"], false);
        assert_eq!(disabled["error"], "packages.open: disabled");
        fixture.shutdown().await;
    }
