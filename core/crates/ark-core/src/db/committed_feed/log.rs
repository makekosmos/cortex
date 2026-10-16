pub fn append_committed_change(
    conn: &Connection,
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
    conn.execute(
        "INSERT OR IGNORE INTO ark_committed_feed(
            entity_type, entity_id, revision, deleted, origin_device_id, origin_sequence
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            entity_type,
            entity_id,
            revision,
            deleted as i64,
            origin_device_id,
            origin_sequence
        ],
    )
    .map_err(|error| error.to_string())?;
    let position = conn
        .query_row(
            "SELECT position FROM ark_committed_feed
         WHERE entity_type = ?1 AND entity_id = ?2 AND revision = ?3",
            params![entity_type, entity_id, revision],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO ark_committed_feed_heads(entity_type, entity_id, position)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
             position = MAX(ark_committed_feed_heads.position, excluded.position)",
        params![entity_type, entity_id, position],
    )
    .map_err(|error| error.to_string())?;
    Ok(position)
}

pub fn committed_feed_snapshot(conn: &Connection) -> Result<FeedSnapshot, String> {
    let (low, high): (Option<i64>, i64) = conn
        .query_row(
            "SELECT MIN(position), COALESCE(MAX(position), 0) FROM ark_committed_feed",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    Ok(FeedSnapshot {
        retained_through: low.map_or(high, |position| position.saturating_sub(1)),
        high_watermark: high,
    })
}

pub fn read_committed_feed_page(
    conn: &Connection,
    after_position: i64,
    high_watermark: i64,
    limit: usize,
) -> Result<Vec<CommittedFeedEntry>, String> {
    if after_position < 0 || high_watermark < after_position {
        return Err("invalid_committed_feed_cursor".into());
    }
    if limit == 0 || limit > MAX_FEED_PAGE_SIZE {
        return Err("committed_feed_page_limit_exceeded".into());
    }
    let snapshot = committed_feed_snapshot(conn)?;
    if after_position < snapshot.retained_through {
        return Err("feed_gap_resync_required".into());
    }
    if high_watermark > snapshot.high_watermark {
        return Err("committed_feed_watermark_from_future".into());
    }
    let mut statement = conn
        .prepare(
            "SELECT position, entity_type, entity_id, revision, deleted,
                    origin_device_id, origin_sequence
             FROM ark_committed_feed
             WHERE position > ?1 AND position <= ?2
             ORDER BY position ASC LIMIT ?3",
        )
        .map_err(|error| error.to_string())?;
    let entries = statement
        .query_map(
            params![after_position, high_watermark, limit as i64],
            |row| {
                Ok(CommittedFeedEntry {
                    position: row.get(0)?,
                    entity_type: row.get(1)?,
                    entity_id: row.get(2)?,
                    revision: row.get(3)?,
                    deleted: row.get::<_, i64>(4)? != 0,
                    origin_device_id: row.get(5)?,
                    origin_sequence: row.get(6)?,
                })
            },
        )
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(entries)
}
