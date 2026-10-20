/// Enforce the sync wire limit before storing a durable inbox receipt.
#[cfg(test)]
pub fn receive_inbox_payload(
    conn: &Connection,
    peer_device_id: &str,
    message_id: &str,
    payload_json: &str,
) -> Result<InboxState, String> {
    if payload_json.len() > MAX_INBOX_PAYLOAD_BYTES {
        return Err("sync_inbox_payload_limit_exceeded".into());
    }
    ark_store::committed_feed::receive_inbox_payload(conn, peer_device_id, message_id, payload_json)
}

#[cfg(test)]
pub fn set_inbox_state(
    conn: &Connection,
    peer_device_id: &str,
    message_id: &str,
    state: InboxState,
) -> Result<(), String> {
    ark_store::committed_feed::set_inbox_state(conn, peer_device_id, message_id, state)
}
