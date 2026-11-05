use std::collections::HashSet;

use ark_sync_contracts::wire::{deserialize_message, serialize_message};
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
