fn require_current_policy_identity(
    conn: &Connection,
    peer_device_id: &str,
    expected: &ark_application::sync_storage::PolicyIdentity,
) -> Result<(), String> {
    let current = ark_store::data_platform::load_accepted_peer_sync_policy(conn, peer_device_id)?
        .map_or_else(
            ark_application::sync_policy::default_sync_policy_identity,
            |policy| policy.identity(),
        );
    if current != *expected {
        return Err("outbound_feed_policy_changed".into());
    }
    Ok(())
}

pub fn begin_outbound_policy_identity(
    conn: &Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
) -> Result<OutboundProgress, String> {
    require_current_policy_identity(conn, peer_device_id, identity)?;
    ark_store::committed_feed::begin_outbound_policy(conn, peer_device_id, identity.revision)
}

pub fn complete_outbound_snapshot_identity(
    conn: &Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
) -> Result<OutboundProgress, String> {
    require_current_policy_identity(conn, peer_device_id, identity)?;
    ark_store::committed_feed::complete_outbound_snapshot(conn, peer_device_id, identity.revision)
}

pub fn record_outbound_scan_identity(
    conn: &Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
    scanned_through: i64,
) -> Result<OutboundProgress, String> {
    require_current_policy_identity(conn, peer_device_id, identity)?;
    ark_store::committed_feed::record_outbound_scan(
        conn,
        peer_device_id,
        identity.revision,
        scanned_through,
    )
}

pub fn acknowledge_outbound_identity(
    conn: &Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
    position: i64,
    ack: OutboundAck,
) -> Result<OutboundProgress, String> {
    require_current_policy_identity(conn, peer_device_id, identity)?;
    ark_store::committed_feed::acknowledge_outbound(
        conn,
        peer_device_id,
        identity.revision,
        position,
        ack,
    )
}
