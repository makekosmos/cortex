use super::*;
#[test]
fn ws_message_limit_accepts_five_minute_dictation_wav() {
    // Regression: 2026-08-19. Base64 WAV used to exceed the 1 MiB WS limit
    // and tungstenite closed the whole Engine connection before dispatch.
    const PCM_BYTES_PER_SECOND: usize = 16_000 * 2;
    const FIVE_MINUTE_WAV_BASE64_BYTES: usize = (44 + PCM_BYTES_PER_SECOND * 300).div_ceil(3) * 4;
    const { assert!(MAX_WS_MESSAGE_BYTES >= FIVE_MINUTE_WAV_BASE64_BYTES + 1024) };
}
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
        panic!("expected accept, got {outcome:?}")
    };
    compatibility
}

fn rejected_code(outcome: HelloOutcome) -> &'static str {
    let HelloOutcome::Reject { code, .. } = outcome else {
        panic!("expected reject, got {outcome:?}")
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
