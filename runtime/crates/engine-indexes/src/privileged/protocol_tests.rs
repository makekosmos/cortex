use super::*;
use std::fs;
use tempfile::TempDir;

const ORIGINAL: &str = "# Copyright (c) Microsoft Corp.\n127.0.0.1 localhost\n";

fn setup() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let hosts = dir.path().join("hosts");
    fs::write(&hosts, ORIGINAL).unwrap();
    (dir, hosts)
}

#[test]
fn parse_hosts_apply_request() {
    let raw = r#"{"op":"hosts_apply","block":"site-block","domains":["tiktok.com"]}"#;
    let req: Request = serde_json::from_str(raw).unwrap();
    assert_eq!(
        req,
        Request::HostsApply {
            block: "site-block".into(),
            domains: vec!["tiktok.com".into()]
        }
    );
}

#[test]
fn parse_simple_requests() {
    assert_eq!(
        serde_json::from_str::<Request>(r#"{"op":"reset"}"#).unwrap(),
        Request::Reset
    );
    assert_eq!(
        serde_json::from_str::<Request>(r#"{"op":"hosts_status"}"#).unwrap(),
        Request::HostsStatus
    );
    assert_eq!(
        serde_json::from_str::<Request>(r#"{"op":"ping"}"#).unwrap(),
        Request::Ping
    );
}

#[test]
fn encode_request_stamps_protocol_version() {
    let wire = encode_request(&Request::Ping);
    let value: Value = serde_json::from_str(&wire).unwrap();
    assert_eq!(value["protocol_version"], PROTOCOL_VERSION);
    assert_eq!(value["op"], "ping");
    // Older services ignore unknown top-level fields — the same wire request
    // still parses as the pre-versioned shape.
}

#[test]
fn invalid_json_returns_error_response() {
    let (_d, hosts) = setup();
    let resp = handle_raw("not json", &hosts);
    assert!(!resp.ok);
    assert!(resp.error.as_deref().unwrap().contains("invalid"));
}

#[test]
fn unknown_op_returns_error_response() {
    let (_d, hosts) = setup();
    let resp = handle_raw(r#"{"op":"obliterate"}"#, &hosts);
    assert!(!resp.ok);
    assert!(resp.error.is_some());
}

#[test]
fn ping_returns_pong_with_version() {
    let (_d, hosts) = setup();
    let resp = handle_raw(r#"{"op":"ping"}"#, &hosts);
    assert!(resp.ok);
    assert_eq!(resp.pong, Some(true));
    assert_eq!(resp.protocol_version, Some(PROTOCOL_VERSION));
}

#[test]
fn newer_request_version_is_refused() {
    let (_d, hosts) = setup();
    let raw = format!(
        r#"{{"op":"ping","protocol_version":{}}}"#,
        PROTOCOL_VERSION + 1
    );
    let resp = handle_raw(&raw, &hosts);
    assert!(!resp.ok);
    assert!(resp.error.as_deref().unwrap().contains("protocol version"));
    assert_eq!(resp.protocol_version, Some(PROTOCOL_VERSION));
}

#[test]
fn client_accepts_older_compatible_service() {
    // The installed service may lag the Engine across updates: a service
    // reporting an older (or pre-versioned = 1) protocol must be accepted.
    assert!(client_accepts(PROTOCOL_VERSION));
    assert!(client_accepts(MIN_PROTOCOL_VERSION));
    assert!(!client_accepts(PROTOCOL_VERSION + 1));
    assert!(!client_accepts(0));
    // Missing field on the wire reads as the first shipped version.
    assert_eq!(
        wire_version(&serde_json::json!({"ok":true})),
        MIN_PROTOCOL_VERSION
    );
    assert_eq!(wire_version(&serde_json::json!({"protocol_version": 1})), 1);
    assert!(client_accepts(wire_version(
        &serde_json::json!({"ok":true})
    )));
}

#[test]
fn apply_dispatch_writes_hosts() {
    let (_d, hosts) = setup();
    let resp = handle_raw(
        r#"{"op":"hosts_apply","block":"site-block","domains":["tiktok.com"]}"#,
        &hosts,
    );
    assert!(resp.ok, "expected ok, got {:?}", resp.error);
    assert_eq!(resp.active_domains.unwrap(), vec!["tiktok.com".to_string()]);
    assert!(fs::read_to_string(&hosts)
        .unwrap()
        .contains("127.0.0.1 tiktok.com"));
}

#[test]
fn hosts_status_returns_all_blocks() {
    let (_d, hosts) = setup();
    handle_raw(
        r#"{"op":"hosts_apply","block":"site-block","domains":["a.com","b.com"]}"#,
        &hosts,
    );
    handle_raw(
        r#"{"op":"hosts_apply","block":"other","domains":["c.com"]}"#,
        &hosts,
    );
    let resp = handle_raw(r#"{"op":"hosts_status"}"#, &hosts);
    assert!(resp.ok);
    let blocks = resp.blocks.unwrap();
    assert_eq!(
        blocks["site-block"],
        vec!["a.com".to_string(), "b.com".to_string()]
    );
    assert_eq!(blocks["other"], vec!["c.com".to_string()]);
}

#[test]
fn remove_clears_only_named_block() {
    let (_d, hosts) = setup();
    handle_raw(
        r#"{"op":"hosts_apply","block":"site-block","domains":["x.com"]}"#,
        &hosts,
    );
    handle_raw(
        r#"{"op":"hosts_apply","block":"other","domains":["y.com"]}"#,
        &hosts,
    );
    let resp = handle_raw(r#"{"op":"hosts_remove","block":"site-block"}"#, &hosts);
    assert!(resp.ok);
    let content = fs::read_to_string(&hosts).unwrap();
    assert!(!content.contains("x.com"));
    assert!(content.contains("y.com"));
}

#[test]
fn reset_clears_everything() {
    let (_d, hosts) = setup();
    handle_raw(
        r#"{"op":"hosts_apply","block":"site-block","domains":["x.com"]}"#,
        &hosts,
    );
    let resp = handle_raw(r#"{"op":"reset"}"#, &hosts);
    assert!(resp.ok);
    assert!(!fs::read_to_string(&hosts).unwrap().contains("engine:"));
}

/// The firewall op is parameterless on the wire by design — the service
/// derives the program path from the pipe client, so nothing a caller sends
/// can widen the grant (KOS-269).
#[cfg(windows)]
#[test]
fn ensure_engine_allow_is_parameterless_and_pipe_only() {
    let req = Request::EnsureEngineAllow;
    let wire = encode_request(&req);
    let value: Value = serde_json::from_str(&wire).unwrap();
    assert_eq!(value["op"], "ensure_engine_allow");
    assert_eq!(value.as_object().unwrap().len(), 2); // op + protocol_version only

    // Direct dispatch (no pipe) must refuse — the handler needs the
    // connection to identify the caller.
    let (_d, hosts) = setup();
    let resp = handle_raw(r#"{"op":"ensure_engine_allow"}"#, &hosts);
    assert!(!resp.ok);
    assert!(resp
        .error
        .as_deref()
        .unwrap()
        .contains("requires a pipe connection"));
}

#[test]
fn response_serializes_without_none_fields() {
    let resp = Response::ok_domains(vec!["a.com".into()]);
    let s = serde_json::to_string(&resp).unwrap();
    assert!(s.contains("\"ok\":true"));
    assert!(s.contains("active_domains"));
    assert!(s.contains("protocol_version"));
    assert!(!s.contains("\"error\""));
    assert!(!s.contains("pong"));
}

#[test]
fn err_response_shape() {
    let resp = Response::err("boom");
    let s = serde_json::to_string(&resp).unwrap();
    assert!(s.contains("\"ok\":false"));
    assert!(s.contains("\"error\":\"boom\""));
    assert!(!s.contains("active_domains"));
}
