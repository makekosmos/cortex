//! KOS-369 wire compatibility: the `pairing_rejected` control message
//! round-trips, and frames an old peer can't parse drop cleanly.
use ark_sync_contracts::wire::{deserialize_message, serialize_message, LanSyncMessage};

/// `pairing_rejected` is a real wire frame (old peers drop it and get their
/// connection closed); an unknown frame type still fails to parse.
#[test]
fn pairing_rejected_message_roundtrips_and_unknown_frames_drop() {
    let msg = LanSyncMessage::PairingRejected {
        device_id: "device-a".to_string(),
    };
    let raw = serialize_message(&msg);
    assert_eq!(deserialize_message(&raw), Some(msg));
    assert!(
        deserialize_message(r#"{"type":"pairing_future_unknown"}"#).is_none(),
        "frames a peer cannot understand must parse-fail, not misapply"
    );
}

#[test]
fn pairing_accepted_is_a_device_identified_control_frame() {
    let msg = LanSyncMessage::PairingAccepted {
        device_id: "device-b".to_string(),
    };
    let raw = serialize_message(&msg);
    assert_eq!(deserialize_message(&raw), Some(msg.clone()));
    assert_eq!(
        ark_sync_contracts::wire::message_origin_device_id(&msg).as_deref(),
        Some("device-b")
    );
}

/// The exact ticket from the KOS-367 report must decode to the reported
/// EndpointId — the wire format was never the problem.
#[test]
fn reported_ticket_decodes() {
    let addr = crate::iroh_transport::from_ticket(concat!(
        "endpointaa2bzzly2lh3s442nsu6ttlbfvozdcyk5xj6qzw7qcf7xbuxv5mxubaa",
        "enuhi5dqom5c6l3fovrtcljrfzzgk3dbpexg4mbonfzg62bonruw42zof4aqacqa",
        "aabndmydaeaj3liztwc3uaqbadakqaky2gzqg"
    ))
    .expect("the reported ticket is a valid EndpointTicket");
    assert_eq!(
        addr.id.to_string(),
        "341ce578d2cfb9739a6ca9e9cd612d5d918b0aedd3e866df808bfb8697af597a"
    );
}
