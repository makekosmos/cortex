use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

mod delivery;
mod log;
#[cfg(test)]
#[path = "committed_feed/persistence_contract_tests.rs"]
mod persistence_contract_tests;
mod progress;
#[cfg(test)]
#[path = "committed_feed/selective_sync_tests.rs"]
mod selective_sync_tests;
#[cfg(test)]
#[path = "committed_feed/test_support.rs"]
mod test_support;

pub use delivery::{read_committed_feed_delivery_page, read_committed_feed_delivery_page_for_peer};
pub use log::{committed_feed_snapshot, read_committed_feed_page};

pub const MAX_FEED_PAGE_SIZE: usize = 128;

pub use progress::{
    acknowledge_outbound, begin_outbound_policy, complete_outbound_snapshot,
    load_outbound_progress, record_outbound_scan,
};

const MAX_PENDING_INBOX_ROWS: i64 = 10_000;
const MAX_PENDING_INBOX_BYTES: i64 = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboxState {
    Received,
    Pending,
    Applied,
    Rejected,
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

/// Persist an entity receipt before application in the caller's transaction.
/// A retry with the same identity but different contents is rejected.
pub fn receive_inbox_payload(
    conn: &Connection,
    peer_device_id: &str,
    message_id: &str,
    payload_json: &str,
) -> Result<InboxState, String> {
    let payload_bytes = payload_json.len();
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

pub fn append_committed_change(
    connection: &Connection,
    entity_type: &str,
    entity_id: &str,
    revision: &str,
    deleted: bool,
    origin_device_id: Option<&str>,
    origin_sequence: Option<u64>,
) -> Result<i64, String> {
    let origin_sequence = origin_sequence
        .map(i64::try_from)
        .transpose()
        .map_err(|_| "feed_origin_sequence_overflow".to_string())?;
    connection
        .execute(
            "INSERT OR IGNORE INTO ark_committed_feed(
                entity_type, entity_id, revision, deleted, origin_device_id, origin_sequence
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                entity_type,
                entity_id,
                revision,
                deleted as i64,
                origin_device_id,
                origin_sequence
            ],
        )
        .map_err(|error| error.to_string())?;
    let position: i64 = connection
        .query_row(
            "SELECT position FROM ark_committed_feed
             WHERE entity_type = ?1 AND entity_id = ?2 AND revision = ?3",
            rusqlite::params![entity_type, entity_id, revision],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO ark_committed_feed_heads(entity_type, entity_id, position)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(entity_type, entity_id) DO UPDATE SET
                 position = MAX(ark_committed_feed_heads.position, excluded.position)",
            rusqlite::params![entity_type, entity_id, position],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO ark_committed_feed_meta(meta_key, meta_value)
             VALUES ('watermark-v1', ?1)
             ON CONFLICT(meta_key) DO UPDATE SET
                 meta_value = MAX(CAST(ark_committed_feed_meta.meta_value AS INTEGER),
                                  CAST(excluded.meta_value AS INTEGER))",
            [position.to_string()],
        )
        .map_err(|error| error.to_string())?;
    Ok(position)
}

pub fn watermark(connection: &Connection) -> Result<i64, String> {
    let persisted: Option<String> = connection
        .query_row(
            "SELECT meta_value FROM ark_committed_feed_meta WHERE meta_key = 'watermark-v1'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let persisted = persisted
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| "invalid_committed_feed_watermark".to_string())
        })
        .transpose()?
        .unwrap_or_default();
    if persisted < 0 {
        return Err("invalid_committed_feed_watermark".into());
    }

    // AUTOINCREMENT preserves a lower bound after feed pruning. Retained rows
    // cover databases created before the sequence and watermark were stored.
    let retained: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(position), 0) FROM ark_committed_feed",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let sequence: Option<i64> = connection
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name = 'ark_committed_feed'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(persisted.max(retained).max(sequence.unwrap_or_default()))
}

pub fn bootstrap_committed_feed_heads(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT ark_feed_heads_bootstrap")
        .map_err(|error| error.to_string())?;
    let result = (|| -> Result<(), String> {
        let bootstrapped = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM ark_committed_feed_meta
                 WHERE meta_key = 'heads-v1')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?
            != 0;
        if !bootstrapped {
            conn.execute(
                "INSERT INTO ark_committed_feed_heads(entity_type, entity_id, position)
                 SELECT entity_type, entity_id, MAX(position)
                 FROM ark_committed_feed WHERE 1 GROUP BY entity_type, entity_id
                 ON CONFLICT(entity_type, entity_id) DO UPDATE SET
                     position = MAX(ark_committed_feed_heads.position, excluded.position)",
                [],
            )
            .map_err(|error| error.to_string())?;
            conn.execute(
                "INSERT INTO ark_committed_feed_meta(meta_key, meta_value)
                 VALUES ('heads-v1', 'complete')",
                [],
            )
            .map_err(|error| error.to_string())?;
        }
        let watermark = watermark(conn)?;
        conn.execute(
            "INSERT INTO ark_committed_feed_meta(meta_key, meta_value)
             VALUES ('watermark-v1', ?1)
             ON CONFLICT(meta_key) DO UPDATE SET
                 meta_value = MAX(CAST(ark_committed_feed_meta.meta_value AS INTEGER),
                                  CAST(excluded.meta_value AS INTEGER))",
            [watermark.to_string()],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE ark_feed_heads_bootstrap")
            .map_err(|error| error.to_string()),
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO ark_feed_heads_bootstrap; RELEASE ark_feed_heads_bootstrap",
            );
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        acknowledge_outbound, append_committed_change, begin_outbound_policy,
        bootstrap_committed_feed_heads, complete_outbound_snapshot, load_outbound_progress,
        receive_inbox_payload, record_outbound_scan, set_inbox_state, watermark, InboxState,
    };
    use rusqlite::Connection;

    #[test]
    fn feed_heads_bootstrap_legacy_rows_once_and_is_idempotent() {
        let conn = Connection::open_in_memory().expect("database opens");
        conn.execute_batch(crate::schema::CREATE_TABLES)
            .expect("store schema initializes");
        conn.execute(
            "INSERT INTO ark_committed_feed(
                position, entity_type, entity_id, revision, deleted
             ) VALUES (1, 'object', 'legacy', 'r1', 0),
                      (2, 'object', 'legacy', 'r2', 0),
                      (3, 'todo', 'other', 'r3', 0)",
            [],
        )
        .expect("feed fixture inserts");

        bootstrap_committed_feed_heads(&conn).expect("legacy feed heads bootstrap");
        let first: Vec<(String, String, i64)> = conn
            .prepare(
                "SELECT entity_type, entity_id, position FROM ark_committed_feed_heads
                 ORDER BY entity_type, entity_id",
            )
            .expect("feed heads load")
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .expect("feed head query executes")
            .collect::<Result<_, _>>()
            .expect("feed head rows load");
        assert_eq!(
            first,
            vec![
                ("object".into(), "legacy".into(), 2),
                ("todo".into(), "other".into(), 3)
            ]
        );

        conn.execute(
            "UPDATE ark_committed_feed_heads SET position = 1
             WHERE entity_type = 'object' AND entity_id = 'legacy'",
            [],
        )
        .expect("feed fixture update");
        bootstrap_committed_feed_heads(&conn).expect("idempotent feed heads bootstrap");
        let position: i64 = conn
            .query_row(
                "SELECT position FROM ark_committed_feed_heads
                 WHERE entity_type = 'object' AND entity_id = 'legacy'",
                [],
                |row| row.get(0),
            )
            .expect("bootstrapped feed head loads");
        assert_eq!(
            position, 1,
            "the completion marker prevents a second rebuild"
        );
    }

    #[test]
    fn watermark_survives_feed_pruning_and_reopen() {
        let directory = crate::test_support::test_temp_dir().expect("temporary directory opens");
        let path = directory.path().join("ark.db");
        let connection = Connection::open(&path).expect("database opens");
        connection
            .execute_batch(
                "CREATE TABLE ark_committed_feed (
                    position INTEGER PRIMARY KEY AUTOINCREMENT,
                    entity_id TEXT NOT NULL
                 );
                 CREATE TABLE ark_committed_feed_meta (
                    meta_key TEXT PRIMARY KEY,
                    meta_value TEXT NOT NULL
                 );
                 INSERT INTO ark_committed_feed(entity_id) VALUES ('a'), ('b');
                 DELETE FROM ark_committed_feed;
                 INSERT INTO ark_committed_feed_meta VALUES ('watermark-v1', '2');",
            )
            .expect("feed schema and pruning initialize");
        drop(connection);

        let connection = Connection::open(&path).expect("database reopens");
        assert_eq!(watermark(&connection).expect("watermark loads"), 2);
    }

    #[test]
    fn inbox_receipt_is_idempotent_and_releases_payload_after_apply() {
        let conn = Connection::open_in_memory().expect("database opens");
        conn.execute_batch(crate::schema::CREATE_TABLES)
            .expect("store schema initializes");

        assert_eq!(
            receive_inbox_payload(&conn, "peer-a", "object:a:r1", "{\"x\":1}")
                .expect("inbox receipt persists"),
            InboxState::Received
        );
        assert_eq!(
            receive_inbox_payload(&conn, "peer-a", "object:a:r1", "{\"x\":1}")
                .expect("identical retry reads receipt"),
            InboxState::Received
        );
        assert_eq!(
            receive_inbox_payload(&conn, "peer-a", "object:a:r1", "{\"x\":2}")
                .expect_err("changed contents cannot reuse identity"),
            "sync_inbox_identity_digest_mismatch"
        );

        set_inbox_state(&conn, "peer-a", "object:a:r1", InboxState::Applied)
            .expect("receipt advances after application");
        assert_eq!(
            receive_inbox_payload(&conn, "peer-a", "object:a:r1", "{\"x\":1}")
                .expect("applied receipt survives retry"),
            InboxState::Applied
        );
        let (payload, payload_bytes): (String, i64) = conn
            .query_row(
                "SELECT payload_json, payload_bytes FROM ark_sync_inbox
                 WHERE peer_device_id = 'peer-a' AND message_id = 'object:a:r1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("applied receipt remains durable");
        assert_eq!((payload.as_str(), payload_bytes), ("", 0));
    }

    #[test]
    fn outbound_progress_persists_distinct_receipt_and_applied_cursors() {
        let conn = Connection::open_in_memory().expect("database opens");
        conn.execute_batch(crate::schema::CREATE_TABLES)
            .expect("store schema initializes");
        append_committed_change(&conn, "object", "a", "r1", false, None, None)
            .expect("feed row persists");

        let started = begin_outbound_policy(&conn, "peer-a", 4).expect("progress starts");
        assert_eq!(started.initial_snapshot.through(), 1);
        assert!(!started.initial_snapshot.is_complete());
        let complete =
            complete_outbound_snapshot(&conn, "peer-a", 4).expect("initial snapshot completes");
        assert!(complete.initial_snapshot.is_complete());
        assert_eq!(complete.applied_through, 1);
        append_committed_change(&conn, "object", "b", "r2", false, None, None)
            .expect("live feed row persists");
        let scanned = record_outbound_scan(&conn, "peer-a", 4, 2).expect("scan advances");
        assert_eq!(scanned.scanned_through, 2);
        let received = acknowledge_outbound(
            &conn,
            "peer-a",
            4,
            2,
            ark_application::sync_storage::OutboundAck::DurableReceipt,
        )
        .expect("durable receipt advances");
        assert_eq!(received.receipt_through, 2);
        assert_eq!(received.applied_through, 1);
        let applied = acknowledge_outbound(
            &conn,
            "peer-a",
            4,
            2,
            ark_application::sync_storage::OutboundAck::Applied,
        )
        .expect("application acknowledgement advances");
        assert_eq!(applied.applied_through, 2);
        assert_eq!(
            load_outbound_progress(&conn, "peer-a").expect("progress reloads"),
            applied
        );
    }
}
