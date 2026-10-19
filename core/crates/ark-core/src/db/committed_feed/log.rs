pub fn committed_feed_snapshot(conn: &Connection) -> Result<FeedSnapshot, String> {
    ark_store::committed_feed::committed_feed_snapshot(conn)
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
