use super::*;
use crate::package_manifest::{PackageKind, PackageManifest, PermissionRequest};

fn manifest(capability: &str, scopes: Vec<&str>) -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: "pkg".into(),
        name: "Pkg".into(),
        version: "1.0.0".into(),
        kind: PackageKind::Source,
        engine_api: ">=1".into(),
        entrypoint: "worker.exe".into(),
        publisher: "kosmos".into(),
        permissions: vec![PermissionRequest {
            capability: capability.into(),
            scopes: scopes.into_iter().map(str::to_owned).collect(),
        }],
    }
}

#[test]
fn token_pid_generation_and_allowlist_are_bound() {
    let (grant, token) = Grant::derive(
        &manifest("ark.read", vec!["get_object"]),
        "h".into(),
        7,
        2,
        "c".into(),
        &[],
    )
    .unwrap();
    assert!(grant.authorize(
        &token,
        7,
        2,
        1,
        1,
        &WorkerMethod::ArkRead,
        Some("get_object")
    ));
    assert!(!grant.authorize(
        "wrong",
        7,
        2,
        1,
        1,
        &WorkerMethod::ArkRead,
        Some("get_object")
    ));
    assert!(!grant.authorize(
        &token,
        8,
        2,
        1,
        1,
        &WorkerMethod::ArkRead,
        Some("get_object")
    ));
    assert!(!grant.authorize(
        &token,
        7,
        3,
        1,
        1,
        &WorkerMethod::ArkRead,
        Some("get_object")
    ));
    assert!(!grant.authorize(&token, 7, 2, 1, 1, &WorkerMethod::ArkRead, None));
    assert!(Grant::derive(
        &manifest("clipboard", vec![]),
        "h".into(),
        1,
        1,
        "c".into(),
        &[]
    )
    .is_err());
}

#[test]
fn dictation_control_authorizes_only_registered_engine_operations() {
    let (grant, token) = Grant::derive(
        &manifest("dictation.control", vec!["dictation.capture.start"]),
        "h".into(),
        7,
        2,
        "c".into(),
        &[],
    )
    .unwrap();
    assert!(grant.authorize(
        &token,
        7,
        2,
        1,
        1,
        &WorkerMethod::ArkRead,
        Some("dictation.capture.start")
    ));
    assert!(!grant.authorize(
        &token,
        7,
        2,
        1,
        1,
        &WorkerMethod::ArkRead,
        Some("dictation.submit_audio")
    ));
    assert!(Grant::derive(
        &manifest("dictation.control", vec!["dictation.submit_audio"]),
        "h".into(),
        1,
        1,
        "c".into(),
        &[]
    )
    .is_err());
}

#[test]
fn network_and_path_scopes_fail_closed() {
    assert!(normalize_origin("http://example.com/").is_err());
    assert!(normalize_origin("https://127.0.0.1/").is_err());
    assert!(normalize_origin("https://[::ffff:127.0.0.1]/").is_err());
    assert!(normalize_origin("https://[fc00::1]/").is_err());
    assert!(normalize_origin("https://user@example.com/").is_err());
    let root = std::env::temp_dir();
    assert!(normalize_path("relative", std::slice::from_ref(&root)).is_err());
    assert!(normalize_path("//server/share", std::slice::from_ref(&root)).is_err());
    assert!(normalize_path(
        &root.join("child").to_string_lossy(),
        std::slice::from_ref(&root)
    )
    .is_ok());
    assert!(normalize_path(
        &root.join("child/../escape").to_string_lossy(),
        std::slice::from_ref(&root)
    )
    .is_err());
}

#[test]
fn empty_path_capabilities_fail_closed() {
    let root = std::env::temp_dir();
    for capability in ["filesystem.read", "filesystem.write", "process.spawn"] {
        assert!(matches!(
            Grant::derive(
                &manifest(capability, vec![]),
                "h".into(),
                1,
                1,
                "c".into(),
                std::slice::from_ref(&root),
            ),
            Err(GrantError::InvalidScope)
        ));
    }
}

#[test]
fn malformed_and_oversized_lines_rejected() {
    assert!(parse_json_line(b"{}").is_err());
    assert!(matches!(
        parse_json_line(
            "{\"method\":\"worker.call\",\"id\":\"1\",\"generation\":1,\"token\":\"x\",\
                    \"operation\":\"ark.read\",\"params\":{}}"
                .as_bytes()
        ),
        Ok(WorkerMessage::Call(_))
    ));
    assert!(parse_json_line(
        "{\"method\":\"ark.read\",\"id\":\"1\",\"generation\":1,\"token\":\"x\",\
                \"operation\":\"ark.read\",\"params\":{}}"
            .as_bytes()
    )
    .is_err());
    assert!(matches!(
        parse_json_line(
            br#"{"method":"worker.event","event":"dictation.error","data":{"retryable":true}}"#
        ),
        Ok(WorkerMessage::Event(_))
    ));
    assert!(matches!(
        parse_json_line(br#"{"method":"worker.error","error":"unknown-operation"}"#),
        Ok(WorkerMessage::Error(_))
    ));
    assert!(parse_json_line(&vec![b'a'; MAX_LINE_BYTES + 1]).is_err());
}

#[test]
fn integration_bootstrap_config_is_metadata_and_handle_only() {
    let value = serde_json::json!({
        "settings": [
            {"key": "username", "label": "Username", "kind": "text"},
            {"key": "api_key", "label": "API key", "kind": "secret"}
        ],
        "values": {"username": "bigfrontend-user"},
        "secret_handles": {
            "api_key": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        },
        "schedule": {"interval_seconds": 3600}
    });
    let mut config: IntegrationBootstrapConfig = serde_json::from_value(value).unwrap();
    assert_eq!(config.account_key, None);
    config.settings[0].required = true;
    config.settings[1].required = true;
    assert!(config.validate().is_ok());
    let wire = serde_json::to_string(&config).unwrap();
    assert!(wire.contains("bigfrontend-user"));
    assert!(wire.contains("0123456789abcdef"));
    assert!(!wire.contains("secret-value"));
    assert!(serde_json::from_str::<IntegrationBootstrapConfig>(
        &wire.replace("secret_handles", "unexpected")
    )
    .is_err());

    let invalid = |values, secret_handles| IntegrationBootstrapConfig {
        account_key: None,
        data_origin: None,
        site_id: None,
        settings: config.settings.clone(),
        values,
        secret_handles,
        schedule: None,
    };
    assert!(invalid(
        [("api_key".into(), "secret-value".into())].into(),
        HashMap::new()
    )
    .validate()
    .is_err());
    assert!(invalid(
        HashMap::new(),
        [(
            "username".into(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into()
        )]
        .into()
    )
    .validate()
    .is_err());
    assert!(IntegrationBootstrapConfig {
        account_key: None,
        data_origin: None,
        site_id: None,
        settings: config.settings.clone(),
        values: HashMap::new(),
        secret_handles: HashMap::new(),
        schedule: None,
    }
    .validate()
    .is_err());
    assert!(IntegrationBootstrapConfig {
        account_key: None,
        data_origin: None,
        site_id: None,
        settings: config.settings.clone(),
        values: [("username".into(), "user".into())].into(),
        secret_handles: HashMap::new(),
        schedule: None,
    }
    .validate()
    .is_err());
    assert!(
        invalid([("other".into(), "value".into())].into(), HashMap::new())
            .validate()
            .is_err()
    );
    assert!(IntegrationBootstrapConfig {
        account_key: Some("not-a-sha256-digest".into()),
        data_origin: None,
        site_id: None,
        settings: config.settings,
        values: HashMap::new(),
        secret_handles: HashMap::new(),
        schedule: None,
    }
    .validate()
    .is_err());
}

#[test]
fn run_message_is_strict_and_method_bound() {
    let run: RunMessage =
        serde_json::from_slice(br#"{"method":"worker.run","generation":7,"run_id":"run-1"}"#)
            .unwrap();
    assert!(run.validate().is_ok());
    assert!(serde_json::from_slice::<RunMessage>(
        br#"{"method":"worker.run","generation":7,"run_id":"run-1","extra":true}"#
    )
    .is_err());
    assert!(
        parse_json_line(br#"{"method":"worker.run","generation":7,"run_id":"run-1"}"#).is_err()
    );
}

#[test]
fn opaque_root_open_is_a_strict_worker_call() {
    let message = parse_json_line(
        concat!(
            r#"{"method":"worker.call","id":"1","generation":7,"token":"token","#,
            r#""operation":"filesystem.root.open","params":{"persistent_grant_id":"#,
            r#""00000000-0000-4000-8000-000000000001"}}"#
        )
        .as_bytes(),
    )
    .unwrap();
    assert!(matches!(
        message,
        WorkerMessage::Call(CallMessage {
            operation: WorkerMethod::FilesystemRootOpen,
            ..
        })
    ));
}
