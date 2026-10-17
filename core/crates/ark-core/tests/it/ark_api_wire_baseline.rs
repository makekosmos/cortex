use std::collections::HashSet;
use std::fs;

use ark_core::integration_replication::{
    encode_hex, IntegrationConfiguration, IntegrationReplicationChange,
    IntegrationReplicationEntity, SignedSyncEnvelope,
};
use ark_sync::protocol::{deserialize_message, serialize_message};
use serde::Deserialize;

#[derive(Deserialize)]
struct WireFixture {
    name: String,
    incoming: String,
    current: String,
}

#[test]
fn lan_wire_v1_fixtures_decode_and_preserve_exact_current_bytes() {
    let fixtures = include_str!("../fixtures/ark-api-wire-v1/lan.jsonl");
    let mut seen = HashSet::new();
    for line in fixtures.lines() {
        let fixture: WireFixture =
            serde_json::from_str(line).expect("LAN fixture should be valid JSON");
        let decoded = deserialize_message(&fixture.incoming)
            .unwrap_or_else(|| panic!("{} did not decode", fixture.name));
        assert_eq!(
            serialize_message(&decoded),
            fixture.current,
            "current bytes for {} changed",
            fixture.name
        );
        let current_decoded = deserialize_message(&fixture.current)
            .unwrap_or_else(|| panic!("current form {} did not decode", fixture.name));
        assert_eq!(
            decoded, current_decoded,
            "semantics for {} changed",
            fixture.name
        );
        assert!(seen.insert(fixture.name), "duplicate LAN fixture");
    }
    assert_eq!(seen.len(), 8, "LAN golden corpus is incomplete");
}

#[test]
fn signed_sync_v1_canonical_signing_bytes_match_fixture() {
    let envelope = SignedSyncEnvelope::new(
        "space-a",
        "node-a",
        "node-b",
        7,
        "message-a",
        vec![IntegrationReplicationChange {
            entity: IntegrationReplicationEntity::IntegrationConfiguration(
                IntegrationConfiguration {
                    integration_id: "integration-a".into(),
                    provider: "fatsecret".into(),
                    account_subject: "account-a".into(),
                    public_scopes: vec!["diary.read".into()],
                    public_settings: serde_json::json!({"region":"us"}),
                    enabled: true,
                    sync_cursor: None,
                    revision: 2,
                    hlc: "2026-08-30T00:00:00.000Z:000002:node-a".into(),
                },
            ),
            vector_hlc: "2026-08-30T00:00:00.000Z:000002:node-a".into(),
        }],
        "",
    );
    let actual = encode_hex(
        &envelope
            .canonical_signing_bytes()
            .expect("fixture envelope should have canonical signing bytes"),
    );
    let expected = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ark-api-wire-v1/signed-canonical.hex"
    ))
    .expect("signed canonical-byte fixture should be readable");
    assert_eq!(actual, expected.trim());
}
