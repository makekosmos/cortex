/// Persist an entity-level inbox receipt before application in the same DB
/// transaction. The identity intentionally excludes the payload digest so a
/// retry with the same revision but changed contents is rejected.
pub fn receive_inbox_payload(
    conn: &Connection,
    peer_device_id: &str,
    message_id: &str,
    payload_json: &str,
) -> Result<InboxState, String> {
    let payload_bytes = payload_json.len();
    if payload_bytes > MAX_INBOX_PAYLOAD_BYTES {
        return Err("sync_inbox_payload_limit_exceeded".into());
    }
    let digest = format!("{:x}", Sha256::digest(payload_json.as_bytes()));
    if let Some((stored_digest, state)) = conn
        .query_row(
            "SELECT payload_sha256, state FROM ark_sync_inbox
             WHERE peer_device_id = ?1 AND message_id = ?2",
            params![peer_device_id, message_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?
    {
        if stored_digest != digest {
            return Err("sync_inbox_identity_digest_mismatch".into());
        }
        return InboxState::parse(&state);
    }
    let (row_count, payload_total): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(payload_bytes), 0) FROM ark_sync_inbox
             WHERE state IN ('received', 'pending')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    if row_count >= MAX_PENDING_INBOX_ROWS
        || payload_total.saturating_add(payload_bytes as i64) > MAX_PENDING_INBOX_BYTES
    {
        return Err("sync_inbox_capacity_exceeded".into());
    }
    conn.execute(
        "INSERT INTO ark_sync_inbox(
            peer_device_id, message_id, payload_sha256, payload_bytes, payload_json, state
         ) VALUES (?1, ?2, ?3, ?4, ?5, 'received')",
        params![
            peer_device_id,
            message_id,
            digest,
            payload_bytes as i64,
            payload_json
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(InboxState::Received)
}

pub fn set_inbox_state(
    conn: &Connection,
    peer_device_id: &str,
    message_id: &str,
    state: InboxState,
) -> Result<(), String> {
    let changed = conn
        .execute(
            concat!(
                "UPDATE ark_sync_inbox SET state = ?3, ",
                "applied_at = CASE WHEN ?3 = 'applied' ",
                "THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE applied_at END, ",
                "payload_json = CASE WHEN ?3 IN ('applied', 'rejected') ",
                "THEN '' ELSE payload_json END, ",
                "payload_bytes = CASE WHEN ?3 IN ('applied', 'rejected') ",
                "THEN 0 ELSE payload_bytes END ",
                "WHERE peer_device_id = ?1 AND message_id = ?2"
            ),
            params![peer_device_id, message_id, state.as_str()],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("sync_inbox_receipt_not_found".into());
    }
    Ok(())
}
