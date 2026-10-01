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
// Compaction rule (explicit):
//   delete a ref iff
//     1) another ref for the same (entity_type, entity_id) carries a newer
//        hlc — the entity still reaches every peer, fresh devices included,
//        through the surviving ref;
//     2) the ref's hlc wall time is older than `USAGE_SYNC_LOG_RETENTION_DAYS`
//        — peers that sync more often than the horizon never see a gap.
//   The newest ref per entity is NEVER deleted: it is the only carrier to
//   peers that have not seen the entity ("unsynced rows are never dropped").
//   Refs to deleted/missing entity rows are never superseded by a newer ref,
//   so they also stay — they are how peers learn about the deletion.
//
// Documented trade-off: a peer whose cursor sits below a deleted seq cannot
// advance its contiguous cursor past the gap; its later pulls keep getting
// the surviving tail. The data stays correct (entities still arrive via the
// surviving refs, dedup by hlc); the 30-day horizon confines the cost to
// peers that have not synced in a month.
//
// Batching: one call = one short write transaction of at most `batch_limit`
// rows. The Engine loops this op with pauses between calls, so the service
// worker never holds the DB through a multi-million-row sweep.

/// Age horizon for compaction: refs younger than this are always kept, so
/// devices that sync regularly observe a gapless stream.
pub const USAGE_SYNC_LOG_RETENTION_DAYS: i64 = 30;

/// Delete one batch of superseded refs. `cutoff_wall_time` is the ISO-8601
/// hlc wall-time prefix (24 chars) bounding which refs are old enough.
/// Returns the number of deleted rows; the caller repeats while the result
/// equals `batch_limit`.
pub fn compact_usage_sync_log(
    conn: &Connection,
    cutoff_wall_time: &str,
    batch_limit: i64,
) -> Result<usize, String> {
    conn.execute(
        "DELETE FROM usage_sync_log
         WHERE (device_id, seq) IN (
             SELECT l.device_id, l.seq
             FROM usage_sync_log l
             WHERE SUBSTR(l.hlc, 1, 24) < ?1
               AND EXISTS (
                   SELECT 1 FROM usage_sync_log n
                   WHERE n.entity_type = l.entity_type
                     AND n.entity_id = l.entity_id
                     AND n.hlc > l.hlc
               )
             LIMIT ?2
         )",
        params![cutoff_wall_time, batch_limit.max(1)],
    )
    .map_err(|e| e.to_string())
}
