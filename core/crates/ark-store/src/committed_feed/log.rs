use ark_application::sync_storage::{CommittedFeedEntry, FeedSnapshot};
use rusqlite::{params, Connection};

pub fn committed_feed_snapshot(conn: &Connection) -> Result<FeedSnapshot, String> {
    let low: Option<i64> = conn
        .query_row("SELECT MIN(position) FROM ark_committed_feed", [], |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    let high = super::watermark(conn)?;
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
    read_committed_feed_page_with_hook(conn, after_position, high_watermark, limit, || {})
}

fn read_committed_feed_page_with_hook(
    conn: &Connection,
    after_position: i64,
    high_watermark: i64,
    limit: usize,
    after_snapshot_check: impl FnOnce(),
) -> Result<Vec<CommittedFeedEntry>, String> {
    conn.execute_batch("SAVEPOINT committed_feed_log_snapshot")
        .map_err(|error| error.to_string())?;
    let result = (|| -> Result<Vec<CommittedFeedEntry>, String> {
        if after_position < 0 || high_watermark < after_position {
            return Err("invalid_committed_feed_cursor".into());
        }
        if limit == 0 || limit > super::MAX_FEED_PAGE_SIZE {
            return Err("committed_feed_page_limit_exceeded".into());
        }
        let snapshot = committed_feed_snapshot(conn)?;
        if after_position < snapshot.retained_through {
            return Err("feed_gap_resync_required".into());
        }
        if high_watermark > snapshot.high_watermark {
            return Err("committed_feed_watermark_from_future".into());
        }
        after_snapshot_check();
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
    })();
    match result {
        Ok(entries) => conn
            .execute_batch("RELEASE committed_feed_log_snapshot")
            .map(|()| entries)
            .map_err(|error| error.to_string()),
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO committed_feed_log_snapshot;
                 RELEASE committed_feed_log_snapshot",
            );
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::read_committed_feed_page_with_hook;
    use crate::committed_feed::append_committed_change;

    #[test]
    fn page_selection_uses_the_snapshot_that_passed_retention_checks() {
        let directory = crate::test_support::test_temp_dir().expect("temporary directory opens");
        let path = directory.path().join("feed-log-snapshot.db");
        let path = path.to_str().expect("temporary path is UTF-8");
        let reader = crate::open::open_db(path).expect("reader opens");
        reader
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("schema initializes");
        append_committed_change(&reader, "object", "selected", "r1", false, None, None)
            .expect("feed entry persists");
        let writer = crate::open::open_db(path).expect("writer opens");

        let page = read_committed_feed_page_with_hook(&reader, 0, 1, 8, || {
            writer
                .execute("DELETE FROM ark_committed_feed WHERE position = 1", [])
                .expect("concurrent pruning commits");
        })
        .expect("page reads from the checked snapshot");

        assert_eq!(page.len(), 1);
        assert_eq!(page[0].entity_id, "selected");
        assert_eq!(page[0].position, 1);
    }
}
