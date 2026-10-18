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
    begin_outbound_policy(conn, peer_device_id, identity.revision)
}

pub fn complete_outbound_snapshot_identity(
    conn: &Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
) -> Result<OutboundProgress, String> {
    require_current_policy_identity(conn, peer_device_id, identity)?;
    complete_outbound_snapshot(conn, peer_device_id, identity.revision)
}

pub fn record_outbound_scan_identity(
    conn: &Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
    scanned_through: i64,
) -> Result<OutboundProgress, String> {
    require_current_policy_identity(conn, peer_device_id, identity)?;
    record_outbound_scan(conn, peer_device_id, identity.revision, scanned_through)
}

pub fn acknowledge_outbound_identity(
    conn: &Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
    position: i64,
    ack: OutboundAck,
) -> Result<OutboundProgress, String> {
    require_current_policy_identity(conn, peer_device_id, identity)?;
    acknowledge_outbound(conn, peer_device_id, identity.revision, position, ack)
}

pub fn begin_outbound_policy(
    conn: &Connection,
    peer_device_id: &str,
    policy_generation: i64,
) -> Result<OutboundProgress, String> {
    if policy_generation < 0 {
        return Err("invalid_sync_policy_generation".into());
    }
    let had_progress = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM ark_feed_outbound_progress WHERE peer_device_id = ?1)",
            [peer_device_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| error.to_string())?
        != 0;
    conn.execute(
        "INSERT INTO ark_feed_outbound_progress(peer_device_id, policy_generation)
         VALUES (?1, ?2)
         ON CONFLICT(peer_device_id) DO UPDATE SET
             policy_generation = excluded.policy_generation,
             scanned_through = CASE
                 WHEN ark_feed_outbound_progress.policy_generation != excluded.policy_generation
                 THEN 0 ELSE ark_feed_outbound_progress.scanned_through END,
             receipt_through = CASE
                 WHEN ark_feed_outbound_progress.policy_generation != excluded.policy_generation
                 THEN 0 ELSE ark_feed_outbound_progress.receipt_through END,
             applied_through = CASE
                 WHEN ark_feed_outbound_progress.policy_generation != excluded.policy_generation
                 THEN 0 ELSE ark_feed_outbound_progress.applied_through END,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        params![peer_device_id, policy_generation],
    )
    .map_err(|error| error.to_string())?;
    let current = conn
        .query_row(
            "SELECT policy_generation, complete FROM ark_feed_initial_snapshot
             WHERE peer_device_id = ?1",
            [peer_device_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)? != 0)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if current.is_none() {
        let watermark = if had_progress {
            // Upgrade of an older progress row: preserve its existing cursor.
            0
        } else {
            committed_feed_snapshot(conn)?.high_watermark
        };
        conn.execute(
            "INSERT INTO ark_feed_initial_snapshot(
                 peer_device_id, policy_generation, watermark, complete
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(peer_device_id) DO UPDATE SET
                 policy_generation = excluded.policy_generation,
                 watermark = excluded.watermark,
                 complete = excluded.complete,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![
                peer_device_id,
                policy_generation,
                watermark,
                had_progress as i64
            ],
        )
        .map_err(|error| error.to_string())?;
    } else if current.is_some_and(|(generation, _)| generation != policy_generation) {
        let previous_complete = current.is_some_and(|(_, complete)| complete);
        let (watermark, complete) = if previous_complete {
            // A selective policy change replays the durable state feed so a
            // newly included type is sent without requiring wire backfill.
            (0, true)
        } else {
            // An interrupted initial backfill restarts under the new policy
            // with a fresh, fixed watermark.
            (committed_feed_snapshot(conn)?.high_watermark, false)
        };
        conn.execute(
            "UPDATE ark_feed_initial_snapshot
             SET policy_generation = ?2, watermark = ?3, complete = ?4,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE peer_device_id = ?1",
            params![
                peer_device_id,
                policy_generation,
                watermark,
                complete as i64
            ],
        )
        .map_err(|error| error.to_string())?;
    }
    load_outbound_progress(conn, peer_device_id)
}

pub fn load_outbound_progress(
    conn: &Connection,
    peer_device_id: &str,
) -> Result<OutboundProgress, String> {
    conn.query_row(
        "SELECT progress.policy_generation, snapshot.watermark, snapshot.complete,
                progress.scanned_through, progress.receipt_through, progress.applied_through
         FROM ark_feed_outbound_progress AS progress
         JOIN ark_feed_initial_snapshot AS snapshot USING (peer_device_id)
         WHERE peer_device_id = ?1",
        [peer_device_id],
        |row| {
            let through = row.get(1)?;
            let snapshot = if row.get::<_, i64>(2)? != 0 {
                InitialSnapshot::Complete { through }
            } else {
                InitialSnapshot::Pending { through }
            };
            Ok(OutboundProgress {
                policy_generation: row.get(0)?,
                initial_snapshot: snapshot,
                scanned_through: row.get(3)?,
                receipt_through: row.get(4)?,
                applied_through: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(|error| error.to_string())?
    .ok_or_else(|| "outbound_feed_peer_not_initialized".into())
}

pub fn complete_outbound_snapshot(
    conn: &Connection,
    peer_device_id: &str,
    policy_generation: i64,
) -> Result<OutboundProgress, String> {
    let progress = load_outbound_progress(conn, peer_device_id)?;
    if progress.policy_generation != policy_generation {
        return Err("outbound_feed_snapshot_policy_changed".into());
    }
    conn.execute(
        "UPDATE ark_feed_initial_snapshot SET complete = 1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE peer_device_id = ?1 AND policy_generation = ?2",
        params![peer_device_id, policy_generation],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "UPDATE ark_feed_outbound_progress
         SET scanned_through = MAX(scanned_through, ?3),
             receipt_through = MAX(receipt_through, ?3),
             applied_through = MAX(applied_through, ?3),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE peer_device_id = ?1 AND policy_generation = ?2",
        params![
            peer_device_id,
            policy_generation,
            progress.initial_snapshot.through()
        ],
    )
    .map_err(|error| error.to_string())?;
    load_outbound_progress(conn, peer_device_id)
}

pub fn record_outbound_scan(
    conn: &Connection,
    peer_device_id: &str,
    policy_generation: i64,
    scanned_through: i64,
) -> Result<OutboundProgress, String> {
    let progress = load_outbound_progress(conn, peer_device_id)?;
    let snapshot = committed_feed_snapshot(conn)?;
    if !progress.initial_snapshot.is_complete()
        || policy_generation != progress.policy_generation
        || scanned_through < progress.scanned_through
        || scanned_through > snapshot.high_watermark
    {
        return Err("invalid_outbound_feed_scan_progress".into());
    }
    conn.execute(
        "UPDATE ark_feed_outbound_progress
         SET scanned_through = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE peer_device_id = ?1 AND policy_generation = ?2",
        params![peer_device_id, policy_generation, scanned_through],
    )
    .map_err(|error| error.to_string())?;
    load_outbound_progress(conn, peer_device_id)
}

pub fn acknowledge_outbound(
    conn: &Connection,
    peer_device_id: &str,
    policy_generation: i64,
    position: i64,
    ack: OutboundAck,
) -> Result<OutboundProgress, String> {
    let progress = load_outbound_progress(conn, peer_device_id)?;
    if !progress.initial_snapshot.is_complete()
        || policy_generation != progress.policy_generation
        || position < 0
        || position > progress.scanned_through
    {
        return Err("invalid_outbound_feed_ack".into());
    }
    let (receipt, applied) = match ack {
        OutboundAck::DurableReceipt => (
            position.max(progress.receipt_through),
            progress.applied_through,
        ),
        OutboundAck::Applied => (
            position.max(progress.receipt_through),
            position.max(progress.applied_through),
        ),
    };
    conn.execute(
        "UPDATE ark_feed_outbound_progress
         SET receipt_through = ?3, applied_through = ?4,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE peer_device_id = ?1 AND policy_generation = ?2",
        params![peer_device_id, policy_generation, receipt, applied],
    )
    .map_err(|error| error.to_string())?;
    load_outbound_progress(conn, peer_device_id)
}

impl InboxState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::Rejected => "rejected",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "received" => Ok(Self::Received),
            "pending" => Ok(Self::Pending),
            "applied" => Ok(Self::Applied),
            "rejected" => Ok(Self::Rejected),
            _ => Err(format!("invalid durable inbox state: {value}")),
        }
    }
}
