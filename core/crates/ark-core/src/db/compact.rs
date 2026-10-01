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
//      origin device — the exact bound `usage_log_complete_through`
//      advertises on the wire: our log head for our own device (own seqs
//      are allocated contiguously, so the journal is complete through the
//      head by construction, and this bound never depends on the version
//      vector surviving intact), our contiguous `@usage:<device>` cursor
//      for foreign origins. A ref above that bound is one we cannot vouch
//      for — deleting it would leave a relay serving claims it cannot
//      prove.
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
    let raw = get_sync_kv(conn, VERSION_VECTOR_KEY)?;
    let vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default();
    let mut cursors = vector
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix("@usage:")
                .and_then(|device_id| {
                    value.parse::<i64>().ok().map(|seq| (device_id.to_string(), seq))
                })
        })
        .collect::<Vec<_>>();
    cursors.sort_unstable();
    Ok(cursors)
}

/// Delete one batch of superseded refs. `own_device_id` identifies refs we
/// authored locally — their bound is the log head, not the version-vector
/// cursor (a wiped/reset `sync_kv` can freeze the cursor while the head
/// keeps advancing, which would pin every own ref above it forever).
/// `cutoff_wall_time` is the ISO-8601 hlc wall-time prefix (24 chars)
/// bounding which refs are old enough. Returns the number of deleted rows;
/// the caller repeats while the result equals `batch_limit`.
pub fn compact_usage_sync_log(
    conn: &Connection,
    own_device_id: &str,
    cutoff_wall_time: &str,
    batch_limit: i64,
) -> Result<usize, String> {
    // The deletion bound is exactly the completeness claim we send peers:
    // one definition, shared with `usage_complete_through` on the wire.
    let mut bounds: Vec<(String, i64)> = usage_log_complete_through(conn, own_device_id)?
        .into_iter()
        .map(|(device_id, seq)| (device_id, seq as i64))
        .collect();
    bounds.sort_unstable();
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

/// Per origin device, the highest seq this node can serve completely —
/// what the sender advertises as `usage_complete_through` on the final
/// sync page so the receiver can raise its cursor past compacted holes.
/// For our own device that is the log head: own seqs are allocated
/// contiguously, so every seq at or below it is either still in the log or
/// was compacted by us. For a foreign origin it is our contiguous
/// `@usage:<device>` cursor — never claimed above it, because refs past
/// our own coverage are exactly the ones the delete rule spares.
pub fn usage_log_complete_through(
    conn: &Connection,
    own_device_id: &str,
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
        let claim = if device_id == own_device_id {
            max_seq.max(0) as u64
        } else {
            cursors.get(&device_id).copied().unwrap_or(0).max(0) as u64
        };
        through.insert(device_id, claim);
    }
    Ok(through)
}
