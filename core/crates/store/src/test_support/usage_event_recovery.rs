use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageEventRecoveryState {
    pub exists: bool,
    pub sequence: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommittedFeedWatermarks {
    pub snapshot: i64,
    pub live: i64,
}

/// Reads the persisted event and origin sequence used by restore recovery tests.
pub struct UsageEventRecoveryFixture;

impl UsageEventRecoveryFixture {
    pub fn inspect_committed_feed_watermarks(
        live_path: &Path,
        snapshot_path: &Path,
    ) -> Result<CommittedFeedWatermarks, String> {
        let live_path = live_path
            .to_str()
            .ok_or_else(|| "usage recovery database path is not UTF-8".to_string())?;
        let snapshot_path = snapshot_path
            .to_str()
            .ok_or_else(|| "usage recovery snapshot path is not UTF-8".to_string())?;
        let live = crate::open::open_db(live_path)?;
        let snapshot = crate::open::open_db(snapshot_path)?;
        Ok(CommittedFeedWatermarks {
            snapshot: crate::committed_feed::watermark(&snapshot)?,
            live: crate::committed_feed::watermark(&live)?,
        })
    }

    pub fn inspect(
        path: &Path,
        device_id: &str,
        event_id: &str,
    ) -> Result<UsageEventRecoveryState, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "usage recovery database path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        let (exists, sequence) = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM usage_events WHERE id = ?2),
                        (SELECT seq FROM usage_sync_log
                         WHERE device_id = ?1 AND entity_type = 'usage_event' AND entity_id = ?2)",
                (device_id, event_id),
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| error.to_string())?;
        Ok(UsageEventRecoveryState { exists, sequence })
    }
}
