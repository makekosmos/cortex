use crate::VERSION_VECTOR_KEY;
use ark_domain::hlc::HLC;
use ark_domain::types::{SyncEntity, VersionVector};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::collections::HashMap;

mod migration;
pub use migration::{ensure_usage_sequence_migrated, USAGE_SEQUENCE_MIGRATION_KEY};

pub fn record_usage_sequence(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: &str,
    seq: u64,
    hlc: &str,
    deleted: bool,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO usage_sync_log
            (device_id, seq, entity_type, entity_id, hlc, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            device_id,
            seq as i64,
            entity_type,
            entity_id,
            hlc,
            deleted as i64
        ],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO usage_sync_heads(device_id, max_seq) VALUES (?1, ?2)
         ON CONFLICT(device_id) DO UPDATE SET max_seq = MAX(max_seq, excluded.max_seq)",
        params![device_id, seq as i64],
    )
    .map_err(|e| e.to_string())?;

    let raw = crate::sync_kv::get_sync_kv(conn, VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default();
    let cursor_key = format!("@usage:{device_id}");
    let mut contiguous = vector
        .get(&cursor_key)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    while conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM usage_sync_log WHERE device_id = ?1 AND seq = ?2
             )",
            params![device_id, contiguous.saturating_add(1) as i64],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0
    {
        contiguous = contiguous.saturating_add(1);
    }
    vector.insert(cursor_key, contiguous.to_string());
    vector.remove(entity_id);
    crate::committed_feed::append_committed_change(
        conn,
        entity_type,
        entity_id,
        hlc,
        deleted,
        Some(device_id),
        Some(seq),
    )?;
    crate::sync_kv::set_sync_kv(
        conn,
        VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|e| e.to_string())?,
    )
}

pub fn usage_origin_for_entity(
    conn: &Connection,
    entity: &SyncEntity,
) -> Result<(String, u64), String> {
    if let (Some(device_id), Some(seq)) = (&entity.origin_device_id, entity.origin_seq) {
        return Ok((device_id.clone(), seq));
    }
    if let Some(existing) = conn
        .query_row(
            "SELECT device_id, seq FROM usage_sync_log
             WHERE entity_type = ?1 AND entity_id = ?2 AND hlc = ?3
             LIMIT 1",
            params![entity.entity_type, entity.id, entity.hlc],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        return Ok((existing.0, existing.1.max(0) as u64));
    }
    let parsed = HLC::from_string(&entity.hlc);
    let source_device_id = if parsed.device_id.is_empty() {
        "legacy-usage".to_string()
    } else {
        parsed.device_id
    };
    let device_id = format!("legacy:{source_device_id}");
    let seq = next_usage_sequence(conn, &device_id)?;
    Ok((device_id, seq))
}

pub fn usage_entity_is_newer(conn: &Connection, entity: &SyncEntity) -> Result<bool, String> {
    let local = conn
        .query_row(
            "SELECT hlc FROM usage_sync_versions
             WHERE entity_type = ?1 AND entity_id = ?2",
            params![entity.entity_type, entity.id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(local
        .as_deref()
        .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc)))
}

pub fn finish_usage_entity_sync(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
    conn.execute(
        "INSERT INTO usage_sync_versions(entity_type, entity_id, hlc)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET hlc = excluded.hlc
         WHERE excluded.hlc > usage_sync_versions.hlc",
        params![entity.entity_type, entity.id, entity.hlc],
    )
    .map_err(|e| e.to_string())?;
    let (device_id, seq) = usage_origin_for_entity(conn, entity)?;
    record_usage_sequence(
        conn,
        &entity.entity_type,
        &entity.id,
        &device_id,
        seq,
        &entity.hlc,
        entity.deleted == Some(true),
    )
}

// ---------------------------------------------------------------------------
// usage_sync_log compaction (KOS-302)
// ---------------------------------------------------------------------------
//
// `usage_sync_log` is not user data — it is the replication journal for usage
// entities: each row is a *ref* `(origin device_id, seq) → (entity_type,
// entity_id)` written on every usage write (session heartbeat ~once a minute
// per app, usage_day span flushes). Nothing ever removed refs, so the table
// grew without bound while the entity it points at changed again a minute
// later.
//
// The only reader is the sync serve path (`load_usage_sequence_page`): a peer
// advertises `@usage:<device>` cursors and gets refs with `seq > cursor`.
// Serving resolves each ref to the entity's *current* row, hlc and tombstone
// state — a peer never needs the same entity twice, so an old ref is fully
// covered ("synced") once a newer ref for the same entity exists.
//
// The catch: `@usage:<device>` cursors are *contiguous* — the receiver's
// `record_usage_sequence` advances its cursor only while `seq = cursor+1`
// exists in its own log. Deleting refs opens holes, and a receiver sitting
// below a hole could never advance past it. That is why the sender pairs
// compaction with `usage_complete_through` (see below): on the final sync
// page it tells the receiver up to which seq each origin device is served
// complete, counting compacted refs as covered, and the receiver raises its
// cursor to that floor.
//
// Compaction rule (explicit) — delete a ref iff ALL of:
//   1) another ref for the same (entity_type, entity_id) carries a newer
//      hlc — the entity still reaches every peer, fresh devices included,
//      through the surviving ref;
//   2) the ref's hlc wall time is older than `USAGE_SYNC_LOG_RETENTION_DAYS`
//      — peers that sync more often than the horizon never see a gap;
//   3) the ref's seq is within what we can serve completely for that
//      origin device, per `usage_log_complete_through`: our contiguous
//      `@usage:<device>` cursor for foreign origins, and for the one
//      device id our usage writes are stamped with — our log head.
//      Own seqs are allocated contiguously, so the journal is complete
//      through the head by construction; the head survives a `sync_kv`
//      reset (clear_all, corruption) that can freeze the vector cursor
//      while own seqs keep advancing — pinning every own ref above it
//      forever. A ref above its bound is one we cannot vouch for —
//      deleting it would leave a relay serving claims it cannot prove.
//   The newest ref per entity is NEVER deleted: it is the only carrier to
//   peers that have not seen the entity ("unsynced rows are never dropped").
//   Refs to deleted/missing entity rows are never superseded by a newer ref,
//   so they also stay — they are how peers learn about the deletion.
//
// Batching: one call = one short write transaction of at most `batch_limit`
// rows. The Engine loops this op with pauses between calls, so the service
// worker never holds the DB through a multi-million-row sweep.

/// Age horizon for compaction: refs younger than this are always kept, so
/// devices that sync regularly observe a gapless stream.
pub const USAGE_SYNC_LOG_RETENTION_DAYS: i64 = 30;

fn usage_cursors(conn: &Connection) -> Result<Vec<(String, i64)>, String> {
    let raw = conn
        .query_row(
            "SELECT value FROM sync_kv WHERE key = ?1",
            [VERSION_VECTOR_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default();
    let mut cursors = vector
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix("@usage:").and_then(|device_id| {
                value
                    .parse::<i64>()
                    .ok()
                    .map(|seq| (device_id.to_string(), seq))
            })
        })
        .collect::<Vec<_>>();
    cursors.sort_unstable();
    Ok(cursors)
}

/// Delete one batch of superseded refs. `own_device_id` is the id our own
/// usage writes carry (`None` → every origin is bounded by its contiguous
/// cursor). `cutoff_wall_time` is the ISO-8601 hlc wall-time prefix
/// (24 chars) bounding which refs are old enough. Returns the number of
/// deleted rows; the caller repeats while the result equals `batch_limit`.
pub fn compact_usage_sync_log(
    conn: &Connection,
    own_device_id: Option<&str>,
    cutoff_wall_time: &str,
    batch_limit: i64,
) -> Result<usize, String> {
    let mut bounds: Vec<(String, i64)> = usage_log_complete_through(conn, own_device_id)?
        .into_iter()
        .map(|(device_id, seq)| (device_id, seq as i64))
        .collect();
    bounds.sort_unstable();
    if bounds.is_empty() {
        return Ok(0);
    }
    let mut sql = String::from(
        "DELETE FROM usage_sync_log
         WHERE (device_id, seq) IN (
             SELECT l.device_id, l.seq
             FROM usage_sync_log l
             WHERE SUBSTR(l.hlc, 1, 24) < ?
               AND l.seq <= CASE l.device_id ",
    );
    let mut values = Vec::<rusqlite::types::Value>::new();
    values.push(cutoff_wall_time.to_string().into());
    for (device_id, bound) in &bounds {
        sql.push_str("WHEN ? THEN ? ");
        values.push(device_id.clone().into());
        values.push((*bound).into());
    }
    sql.push_str(
        "ELSE 0 END
               AND EXISTS (
                   SELECT 1 FROM usage_sync_log n
                   WHERE n.entity_type = l.entity_type
                     AND n.entity_id = l.entity_id
                     AND n.hlc > l.hlc
               )
             LIMIT ?
         )",
    );
    values.push(batch_limit.max(1).into());
    conn.execute(&sql, params_from_iter(values))
        .map_err(|e| e.to_string())
}

/// Per origin device, the highest seq this node can serve completely.
/// Default bound is our contiguous `@usage:<device>` cursor — compaction
/// never deletes above it, so the cursor is a provable completeness claim.
/// `Some(own)` additionally bounds that device by its log head: own seqs
/// are allocated contiguously, and the head survives a `sync_kv` reset
/// that can freeze the vector cursor. Compaction passes the id its writes
/// are stamped with; the sync wire passes `None` — cursor-only claims are
/// always a safe lower bound, including for migration backfill stamped
/// with the sync device id (sync startup runs `ensure_usage_sequence_migrated`),
/// whose cursor is contiguous by the same construction.
pub fn usage_log_complete_through(
    conn: &Connection,
    own_device_id: Option<&str>,
) -> Result<HashMap<String, u64>, String> {
    let cursors: HashMap<String, i64> = usage_cursors(conn)?.into_iter().collect();
    let mut statement = conn
        .prepare("SELECT device_id, max_seq FROM usage_sync_heads")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut through = HashMap::with_capacity(rows.len());
    for (device_id, max_seq) in rows {
        let claim = if Some(device_id.as_str()) == own_device_id {
            max_seq.max(0) as u64
        } else {
            cursors.get(&device_id).copied().unwrap_or(0).max(0) as u64
        };
        through.insert(device_id, claim);
    }
    Ok(through)
}

#[cfg(test)]
#[path = "usage_sync_log_cursor_tests.rs"]
mod cursor_tests;

pub fn next_usage_sequence(connection: &Connection, device_id: &str) -> Result<u64, String> {
    connection
        .query_row(
            "INSERT INTO usage_sync_heads(device_id, max_seq) VALUES (?1, 1)
             ON CONFLICT(device_id) DO UPDATE SET max_seq = max_seq + 1
             RETURNING max_seq",
            params![device_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value.max(0) as u64)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_usage_sequence_migrated, usage_log_complete_through, USAGE_SEQUENCE_MIGRATION_KEY,
    };
    use crate::{schema::CREATE_TABLES, sync_kv};
    use ark_domain::types::VersionVector;
    use rusqlite::Connection;

    #[test]
    fn complete_through_uses_the_persisted_sync_cursor() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(concat!(
                "CREATE TABLE sync_kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);\n",
                "CREATE TABLE usage_sync_heads (\n",
                "    device_id TEXT PRIMARY KEY, max_seq INTEGER NOT NULL\n",
                ");\n",
                "INSERT INTO sync_kv VALUES ",
                "('lan_sync.version_vector', '{\"@usage:remote\":\"7\"}');\n",
                "INSERT INTO usage_sync_heads VALUES ('remote', 9);",
            ))
            .expect("usage sync schema initializes");

        let bounds =
            usage_log_complete_through(&connection, None).expect("complete-through bounds load");

        assert_eq!(bounds.get("remote"), Some(&7));
    }

    fn migration_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(CREATE_TABLES)
            .expect("application schema creates");
        connection
    }

    #[test]
    fn sequence_backfill_is_ordered_atomic_and_idempotent() {
        let connection = migration_connection();
        connection
            .execute_batch(
                "INSERT INTO usage_sessions
                    (id, tracked_app_id, device_id, device_name, platform, started_at,
                     process_name, exe_path)
                 VALUES ('session-z', 'app', 'device', 'Device', 'test', 'now', 'p', 'p'),
                        ('session-a', 'app', 'device', 'Device', 'test', 'now', 'p', 'p');
                 INSERT INTO usage_events
                    (id, tracked_app_id, device_id, device_name, platform, occurred_at,
                     kind, process_name, exe_path)
                 VALUES ('event-a', 'app', 'device', 'Device', 'test', 'now', 'focus', 'p', 'p');",
            )
            .expect("legacy usage rows create");
        sync_kv::set_sync_kv(
            &connection,
            crate::VERSION_VECTOR_KEY,
            r#"{"session-a":"legacy","event-a":"legacy","keep":"revision"}"#,
        )
        .expect("legacy vector persists");

        ensure_usage_sequence_migrated(&connection, "source-device")
            .expect("usage sequence backfills");

        let refs = connection
            .prepare(
                "SELECT seq, entity_type, entity_id, hlc FROM usage_sync_log
                 WHERE device_id = 'source-device' ORDER BY seq",
            )
            .expect("backfill rows query prepares")
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .expect("backfill rows read")
            .collect::<Result<Vec<_>, _>>()
            .expect("backfill rows collect");
        assert_eq!(
            refs.iter()
                .map(|(seq, entity_type, id, _)| (*seq, entity_type.as_str(), id.as_str()))
                .collect::<Vec<_>>(),
            [
                (1, "usage_event", "event-a"),
                (2, "usage_session", "session-a"),
                (3, "usage_session", "session-z"),
            ]
        );
        assert!(refs
            .iter()
            .all(|(_, _, _, hlc)| hlc.ends_with("source-device")));
        let vector: VersionVector = serde_json::from_str(
            &sync_kv::get_sync_kv(&connection, crate::VERSION_VECTOR_KEY)
                .expect("vector reads")
                .expect("vector exists"),
        )
        .expect("vector parses");
        assert_eq!(
            vector.get("@usage:source-device").map(String::as_str),
            Some("3")
        );
        assert_eq!(vector.get("keep").map(String::as_str), Some("revision"));
        assert!(!vector.contains_key("event-a"));
        assert!(!vector.contains_key("session-a"));

        ensure_usage_sequence_migrated(&connection, "source-device")
            .expect("repeated migration succeeds");
        let row_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM usage_sync_log WHERE device_id = 'source-device'",
                [],
                |row| row.get(0),
            )
            .expect("backfill count reads");
        assert_eq!(row_count, 3);
        assert_eq!(
            sync_kv::get_sync_kv(&connection, USAGE_SEQUENCE_MIGRATION_KEY)
                .expect("migration marker reads")
                .as_deref(),
            Some("1")
        );
    }

    #[test]
    fn sequence_backfill_failure_rolls_back_rows_and_marker() {
        let connection = migration_connection();
        connection
            .execute(
                "INSERT INTO usage_days(device_id, day, updated_at)\
                 VALUES ('device', 'day', 'now')",
                [],
            )
            .expect("legacy usage day creates");
        connection
            .execute_batch(
                "CREATE TRIGGER reject_usage_backfill
                 BEFORE INSERT ON usage_sync_log
                 BEGIN SELECT RAISE(ABORT, 'injected backfill failure'); END;",
            )
            .expect("failure trigger creates");

        assert!(ensure_usage_sequence_migrated(&connection, "source-device").is_err());
        let versions: i64 = connection
            .query_row("SELECT COUNT(*) FROM usage_sync_versions", [], |row| {
                row.get(0)
            })
            .expect("usage versions count reads");
        assert_eq!(versions, 0);
        assert_eq!(
            sync_kv::get_sync_kv(&connection, USAGE_SEQUENCE_MIGRATION_KEY)
                .expect("migration marker reads"),
            None
        );
    }
}
