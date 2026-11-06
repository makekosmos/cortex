use super::*;

pub(super) fn valid_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
    headers.insert(
        CLIENT_PID_HEADER,
        std::process::id().to_string().parse().unwrap(),
    );
    headers.insert(API_VERSION_HEADER, API_VERSION.parse().unwrap());
    headers
}

#[test]
pub(super) fn http_auth_requires_token_pid_and_compatible_version() {
    let token = "a".repeat(64);
    assert!(authenticate(&valid_headers(&token), &token).is_ok());

    let mut missing_pid = valid_headers(&token);
    missing_pid.remove(CLIENT_PID_HEADER);
    assert_eq!(
        authenticate(&missing_pid, &token)
            .expect_err("missing PID must fail")
            .status(),
        StatusCode::FORBIDDEN
    );

    let mut wrong_version = valid_headers(&token);
    wrong_version.insert(API_VERSION_HEADER, "2.0.0".parse().unwrap());
    assert_eq!(
        authenticate(&wrong_version, &token)
            .expect_err("major mismatch must fail")
            .status(),
        StatusCode::UPGRADE_REQUIRED
    );

    assert_eq!(
        authenticate(&valid_headers("b"), &token)
            .expect_err("wrong token must fail")
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

pub(super) fn app_test_grant() -> LaunchGrant {
    LaunchGrant {
        package_id: "com.kosmos.app".into(),
        package_version: "1.0.0".into(),
        manifest_digest: "digest".into(),
        rules: vec![crate::runtime_grants::GrantRule {
            type_id: "core.note".into(),
            versions: vec!["0.1.0".into()],
            actions: ["read", "create", "update", "delete", "subscribe", "link"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            fields_read: vec!["title".into(), "props.description".into()],
            fields_write: vec!["title".into(), "props.description".into()],
            relations_read: vec!["related".into()],
            relations_write: vec!["related".into()],
        }],
        capabilities: vec![],
    }
}

#[test]
pub(super) fn launch_scoped_dictation_rpc_requires_the_exact_grant_operation() {
    let request = || json!({"operation": "dictation.get_state", "params": {}});
    assert!(parse_app_rpc(request(), &app_test_grant()).is_err());

    let mut grant = app_test_grant();
    grant
        .capabilities
        .push(crate::runtime_grants::ScopedCapability::Dictation {
            operations: vec!["dictation.get_state".into()],
        });
    assert_eq!(
        parse_app_rpc(request(), &grant)
            .expect("granted dictation operation")
            .1,
        "dictation.get_state"
    );
    assert!(parse_app_rpc(
        json!({"operation": "dictation.start_recording", "params": {}}),
        &grant,
    )
    .is_err());
    assert!(parse_app_rpc(
        json!({"operation": "dictation.submit_audio", "params": {}}),
        &grant,
    )
    .is_err());
}

pub(super) fn app_network_grant(scopes: &[&str]) -> LaunchGrant {
    LaunchGrant {
        package_id: "com.kosmos.memoria".into(),
        package_version: "0.6.9".into(),
        manifest_digest: "digest".into(),
        rules: vec![],
        capabilities: vec![crate::runtime_grants::ScopedCapability::AppNetwork {
            scopes: scopes.iter().map(|scope| scope.to_string()).collect(),
        }],
    }
}

#[test]
pub(super) fn launch_scoped_app_network_rpc_requires_named_scope() {
    let base = app_test_grant();
    assert!(parse_app_rpc(
        json!({"operation": "bookMetadata.lookupIsbn", "params": {}}),
        &base,
    )
    .is_err());

    let grant = app_network_grant(&["bookMetadata"]);
    for operation in ["bookMetadata.lookupIsbn", "bookMetadata.fetchPage"] {
        assert_eq!(
            parse_app_rpc(json!({"operation": operation, "params": {}}), &grant)
                .expect("granted operation")
                .1,
            operation
        );
    }
    for operation in ["images.fetch", "images.dominantColor", "images.storeCover"] {
        assert!(
            parse_app_rpc(json!({"operation": operation, "params": {}}), &grant).is_err(),
            "missing images scope must deny {operation}"
        );
    }
    for operation in ["bookMetadata.evil", "images.deleteAll"] {
        assert!(
            parse_app_rpc(json!({"operation": operation, "params": {}}), &grant).is_err(),
            "unknown op must be denied: {operation}"
        );
    }
}

#[tokio::test]
pub(super) async fn launch_scoped_app_network_authorization_denies_missing_scope() {
    let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
        Box::pin(async { Ok(json!({"ok": true, "data": null})) })
    }));
    let params = json!({"url": "https://example.com/cover.png"});
    let forwarded = authorize_app_request(
        "images.fetch",
        params.clone(),
        &app_network_grant(&["images"]),
        &dispatcher,
        &DispatchClient::default(),
    )
    .await
    .expect("granted images scope");
    assert_eq!(forwarded, params);
    assert!(authorize_app_request(
        "images.fetch",
        params.clone(),
        &app_network_grant(&["bookMetadata"]),
        &dispatcher,
        &DispatchClient::default(),
    )
    .await
    .is_err());
    let mut worker = app_network_grant(&[]);
    worker
        .capabilities
        .push(crate::runtime_grants::ScopedCapability::WorkerInvoke {
            operations: vec!["images.*".into()],
        });
    assert!(authorize_app_request(
        "images.fetch",
        params,
        &worker,
        &dispatcher,
        &DispatchClient::default()
    )
    .await
    .is_err());
}

pub(super) fn agents_test_grant() -> LaunchGrant {
    LaunchGrant {
        package_id: "com.kosmos.daedalus".into(),
        package_version: "0.1.0".into(),
        manifest_digest: "digest".into(),
        rules: vec![],
        capabilities: vec![crate::runtime_grants::ScopedCapability::Agents {
            operations: vec![
                "agents.projects.list".into(),
                "agents.sessions.send".into(),
                "agents.models.list".into(),
            ],
        }],
    }
}

#[test]
pub(super) fn launch_scoped_agents_rpc_requires_exact_declared_operation() {
    let mut grant = agents_test_grant();
    grant
        .capabilities
        .push(crate::runtime_grants::ScopedCapability::WorkerInvoke {
            operations: vec!["agents.*".into()],
        });
    let parsed = parse_app_rpc(
        json!({"operation":"agents.projects.list", "params":{"include_archived":true}}),
        &grant,
    )
    .expect("declared read operation");
    assert_eq!(parsed.1, "agents.projects.list");
    assert_eq!(parsed.2["include_archived"], true);

    for operation in [
        "agents.sessions.create",
        "agents.sessions.list",
        "agents.unknown",
        "agents.projects.list_all",
        "focus.list_blocklists",
    ] {
        assert!(
            parse_app_rpc(json!({"operation":operation, "params":{}}), &grant).is_err(),
            "operation must be denied: {operation}"
        );
    }
}

#[tokio::test]
pub(super) async fn launch_scoped_agents_authorization_forwards_exact_operation_and_params() {
    let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
        Box::pin(async { Ok(json!({"ok": true, "data": null})) })
    }));
    let params = json!({"session_id":"session-1", "text":"hello"});
    let forwarded = authorize_app_request(
        "agents.sessions.send",
        params.clone(),
        &agents_test_grant(),
        &dispatcher,
        &DispatchClient::default(),
    )
    .await
    .expect("declared write operation");
    assert_eq!(forwarded, params);
    assert!(authorize_app_request(
        "agents.sessions.create",
        json!({"project_id":"p", "prompt":"no"}),
        &agents_test_grant(),
        &dispatcher,
        &DispatchClient::default(),
    )
    .await
    .is_err());
}
