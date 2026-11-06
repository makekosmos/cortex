use super::*;

#[tokio::test]
pub(super) async fn native_directory_grant_returns_only_an_opaque_package_bound_handle() {
    let dir = tempfile::tempdir().expect("tempdir");
    let selected = dir.path().join("Selected Games");
    std::fs::create_dir(&selected).expect("selected directory");
    std::fs::write(selected.join("save.dat"), b"save").expect("save file");
    let token = "a".repeat(64);
    let service =
        Arc::new(crate::package_service::tests::enabled_filesystem_app_service(dir.path()));
    let grants = service.grant_authority();
    let server = EngineApiServer::bind(
        token.clone(),
        9,
        Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
        "00000000-0000-4000-8000-000000000001".into(),
        service,
        test_dispatcher(),
    )
    .await
    .expect("server");
    let port = server.port();
    let server_shutdown_handle = server.shutdown_handle();
    let task = tokio::spawn(server.run());
    let launch = response_json(
        &raw_http(
            port,
            &request(
                &token,
                "POST",
                "/v1/apps/launch",
                r#"{"id":"com.kosmos.demo"}"#,
            ),
        )
        .await,
    );
    let launch_id = launch["data"]["launch_id"].as_str().unwrap();
    let launch_token = launch["data"]["broker_token"].as_str().unwrap();
    let body = serde_json::json!({"selected_root": selected}).to_string();
    let base = request_with_client(
        &token,
        "POST",
        &format!("/v1/apps/launch/{launch_id}/grants/directory"),
        &body,
        "desktop-host",
        API_VERSION,
    );
    let request = base.replace(
        "Content-Length:",
        &format!("X-Kosmos-Launch-Token: {launch_token}\r\nContent-Length:"),
    );
    let response = raw_http(port, &request).await;
    let value = response_json(&response);
    assert_eq!(value["data"]["label"], "Selected Games");
    assert!(!response.contains(&selected.to_string_lossy().to_string()));
    let persistent = value["data"]["persistentGrantId"].as_str().unwrap();
    let owner = crate::grant_authority::GrantOwner {
        session_id: "worker".into(),
        generation: 2,
        connection_id: 1,
    };
    let (grant_id, _) = grants
        .reopen(&owner, persistent, "com.kosmos.demo")
        .expect("persistent grant reopens for package");
    assert_eq!(
        grants
            .read(&grant_id, &owner, "com.kosmos.demo", &["save.dat"], 16,)
            .unwrap(),
        b"save"
    );
    // Drain connection/request tasks so nothing holds test
    // fixture files past the tempdir cleanup.
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
}

#[tokio::test]
pub(super) async fn lifecycle_cleanup_purges_an_idle_lease_without_a_registry_request() {
    let dir = tempfile::tempdir().expect("tempdir");
    let token = "a".repeat(64);
    let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
    let server = EngineApiServer::bind_with_test_limits(
        token.clone(),
        9,
        Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
        "00000000-0000-4000-8000-000000000001".into(),
        Arc::new(service),
        Duration::from_secs(300),
        2,
        Duration::from_millis(5),
    )
    .await
    .expect("server");
    let port = server.port();
    let leases = Arc::clone(&server.launch_leases);
    let server_shutdown_handle = server.shutdown_handle();
    let task = tokio::spawn(server.run());
    let launched = response_json(
        &raw_http(
            port,
            &request(
                &token,
                "POST",
                "/v1/apps/launch",
                r#"{"id":"com.kosmos.demo"}"#,
            ),
        )
        .await,
    );
    let path = launched["data"]["launch_url"]
        .as_str()
        .expect("url")
        .split_once(&format!(":{port}"))
        .expect("loopback path")
        .1
        .to_owned();
    let launch_id = launched["data"]["launch_id"].as_str().expect("launch id");

    expire_launch_for_test(&leases, launch_id);
    wait_for_lease_count(&leases, 0).await;
    assert!(raw_http(
        port,
        &format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n")
    )
    .await
    .starts_with("HTTP/1.1 404"));
    // Drain connection/request tasks so nothing holds test
    // fixture files past the tempdir cleanup.
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
}

#[tokio::test]
pub(super) async fn http_capacity_preserves_live_assets_and_accepts_a_launch_after_lifecycle_purge()
{
    let dir = tempfile::tempdir().expect("tempdir");
    let token = "a".repeat(64);
    let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
    let server = EngineApiServer::bind_with_test_limits(
        token.clone(),
        9,
        Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
        "00000000-0000-4000-8000-000000000001".into(),
        Arc::new(service),
        Duration::from_secs(300),
        2,
        Duration::from_millis(5),
    )
    .await
    .expect("server");
    let port = server.port();
    let leases = Arc::clone(&server.launch_leases);
    let server_shutdown_handle = server.shutdown_handle();
    let task = tokio::spawn(server.run());
    let launch = || {
        request(
            &token,
            "POST",
            "/v1/apps/launch",
            r#"{"id":"com.kosmos.demo"}"#,
        )
    };
    let first = response_json(&raw_http(port, &launch()).await);
    let first_path = first["data"]["launch_url"]
        .as_str()
        .expect("first url")
        .split_once(&format!(":{port}"))
        .expect("first path")
        .1
        .to_owned();
    let first_id = first["data"]["launch_id"]
        .as_str()
        .expect("first launch id")
        .to_owned();
    let second = response_json(&raw_http(port, &launch()).await);
    let second_path = second["data"]["launch_url"]
        .as_str()
        .expect("second url")
        .split_once(&format!(":{port}"))
        .expect("second path")
        .1
        .to_owned();
    assert!(raw_http(port, &launch()).await.starts_with("HTTP/1.1 429"));
    for path in [&first_path, &second_path] {
        assert!(raw_http(
            port,
            &format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200"));
    }

    expire_launch_for_test(&leases, &first_id);
    wait_for_lease_count(&leases, 1).await;
    assert!(raw_http(
        port,
        &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
    )
    .await
    .starts_with("HTTP/1.1 404"));
    assert!(raw_http(
        port,
        &format!("GET {second_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
    )
    .await
    .starts_with("HTTP/1.1 200"));
    assert!(raw_http(port, &launch()).await.starts_with("HTTP/1.1 200"));
    assert!(raw_http(
        port,
        &format!("GET {second_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
    )
    .await
    .starts_with("HTTP/1.1 200"));
    // Drain connection/request tasks so nothing holds test
    // fixture files past the tempdir cleanup.
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
}

pub(super) fn expire_launch_for_test(leases: &Arc<Mutex<LaunchLeaseRegistry>>, launch_id: &str) {
    leases
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .expire(launch_id);
}

pub(super) async fn wait_for_lease_count(
    leases: &Arc<Mutex<LaunchLeaseRegistry>>,
    expected: usize,
) {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let count = leases
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len();
            if count == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("lifecycle cleanup deadline");
}
