use rusqlite::{params, Connection};
use std::time::Duration;

use crate::db::UsageDb;
use crate::model::{
    duration_to_ms, AppUsageTarget, DeviceProfile, IdleUsage, UsageKind, UsageSnapshot,
    UsageTarget,
};

#[derive(Clone, Debug, Default)]
struct TrackerState {
    active: Option<ActiveSession>,
    session_sequence: u64,
    event_sequence: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveSession {
    session_id: String,
    key: SessionKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SessionKey {
    Foreground { app_id: String, pid: u32 },
    Idle,
}

impl SessionKey {
    fn from_target(target: &UsageTarget) -> Self {
        match target {
            UsageTarget::Foreground(app) => SessionKey::Foreground {
                app_id: app.app_id.clone(),
                pid: app.pid,
            },
            UsageTarget::Idle(_) => SessionKey::Idle,
        }
    }
}

pub struct UsageTracker {
    pub(crate) db: UsageDb,
    state: TrackerState,
}

impl UsageTracker {
    pub fn new(db: UsageDb) -> Self {
        Self {
            db,
            state: TrackerState::default(),
        }
    }

    pub fn process_snapshot(
        &mut self,
        snapshot: UsageSnapshot,
        elapsed: Duration,
    ) -> Result<(), String> {
        let sample_ms = duration_to_ms(elapsed).max(1);
        let mut next_state = self.state.clone();

        let result = self.db.with_transaction(|conn| {
            Self::process_snapshot_tx(conn, &mut next_state, snapshot, sample_ms)
        });

        if result.is_ok() {
            self.state = next_state;
        }

        result.map_err(|error| error.to_string())
    }

    fn process_snapshot_tx(
        conn: &Connection,
        state: &mut TrackerState,
        snapshot: UsageSnapshot,
        sample_ms: i64,
    ) -> rusqlite::Result<()> {
        let UsageSnapshot {
            observed_at,
            device,
            target,
        } = snapshot;
        let observed_at = observed_at.to_rfc3339();

        upsert_device(conn, &device, &observed_at)?;

        let current_key = SessionKey::from_target(&target);
        match target {
            UsageTarget::Foreground(app) => {
                upsert_app(conn, &app, &observed_at)?;
                process_foreground_target(
                    conn,
                    state,
                    &device,
                    &observed_at,
                    current_key,
                    sample_ms,
                    app,
                )?;
            }
            UsageTarget::Idle(idle) => {
                process_idle_target(
                    conn,
                    state,
                    &device,
                    &observed_at,
                    current_key,
                    sample_ms,
                    idle,
                )?;
            }
        }

        Ok(())
    }
}

fn process_foreground_target(
    conn: &Connection,
    state: &mut TrackerState,
    device: &DeviceProfile,
    observed_at: &str,
    current_key: SessionKey,
    sample_ms: i64,
    app: AppUsageTarget,
) -> rusqlite::Result<()> {
    if let Some(active) = state.active.clone() {
        if active.key == current_key {
            update_session(conn, &active.session_id, observed_at, sample_ms, Some(&app))?;
            append_event(
                conn,
                state,
                observed_at,
                device,
                &active.session_id,
                sample_ms,
                Some(&app),
                UsageKind::Foreground,
                "sample",
            )?;
            return Ok(());
        }

        close_session(conn, &active.session_id, observed_at)?;
    }

    let session_id = next_session_id(state, observed_at);
    insert_session(
        conn,
        &session_id,
        device,
        observed_at,
        sample_ms,
        Some(&app),
        UsageKind::Foreground,
    )?;
    state.active = Some(ActiveSession {
        session_id: session_id.clone(),
        key: current_key,
    });
    append_event(
        conn,
        state,
        observed_at,
        device,
        &session_id,
        sample_ms,
        Some(&app),
        UsageKind::Foreground,
        "sample",
    )?;
    Ok(())
}

fn process_idle_target(
    conn: &Connection,
    state: &mut TrackerState,
    device: &DeviceProfile,
    observed_at: &str,
    current_key: SessionKey,
    sample_ms: i64,
    _idle: IdleUsage,
) -> rusqlite::Result<()> {
    if let Some(active) = state.active.clone() {
        if active.key == current_key {
            update_session(conn, &active.session_id, observed_at, sample_ms, None)?;
            append_event(
                conn,
                state,
                observed_at,
                device,
                &active.session_id,
                sample_ms,
                None,
                UsageKind::Idle,
                "sample",
            )?;
            return Ok(());
        }

        close_session(conn, &active.session_id, observed_at)?;
    }

    let session_id = next_session_id(state, observed_at);
    insert_session(
        conn,
        &session_id,
        device,
        observed_at,
        sample_ms,
        None,
        UsageKind::Idle,
    )?;
    state.active = Some(ActiveSession {
        session_id: session_id.clone(),
        key: current_key,
    });
    append_event(
        conn,
        state,
        observed_at,
        device,
        &session_id,
        sample_ms,
        None,
        UsageKind::Idle,
        "sample",
    )?;
    Ok(())
}

fn next_session_id(state: &mut TrackerState, observed_at: &str) -> String {
    state.session_sequence = state.session_sequence.saturating_add(1);
    format!(
        "usage-session-{}-{}",
        observed_at.replace([':', '-', 'T', 'Z'], ""),
        state.session_sequence
    )
}

fn next_event_id(state: &mut TrackerState, observed_at: &str) -> String {
    state.event_sequence = state.event_sequence.saturating_add(1);
    format!(
        "usage-event-{}-{}",
        observed_at.replace([':', '-', 'T', 'Z'], ""),
        state.event_sequence
    )
}

fn upsert_device(conn: &Connection, device: &DeviceProfile, observed_at: &str) -> rusqlite::Result<()> {
    conn.execute(
        r#"
        INSERT INTO usage_devices (
            id, device_name, platform, first_seen_at, last_seen_at, metadata_json
        )
        VALUES (?1, ?2, ?3, ?4, ?4, NULL)
        ON CONFLICT(id) DO UPDATE SET
            device_name = excluded.device_name,
            platform = excluded.platform,
            last_seen_at = excluded.last_seen_at
        "#,
        params![device.device_id, device.device_name, device.platform, observed_at],
    )?;
    Ok(())
}

fn upsert_app(conn: &Connection, app: &AppUsageTarget, observed_at: &str) -> rusqlite::Result<()> {
    let executable_path = app
        .executable_path
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());

    conn.execute(
        r#"
        INSERT INTO usage_apps (
            id,
            platform,
            display_name,
            process_name,
            normalized_exe_path,
            executable_path,
            first_seen_at,
            last_seen_at,
            metadata_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, NULL)
        ON CONFLICT(id) DO UPDATE SET
            platform = excluded.platform,
            display_name = excluded.display_name,
            process_name = excluded.process_name,
            normalized_exe_path = excluded.normalized_exe_path,
            executable_path = excluded.executable_path,
            last_seen_at = excluded.last_seen_at
        "#,
        params![
            app.app_id,
            crate::model::PLATFORM_WINDOWS,
            app.display_name,
            app.process_name,
            app.normalized_exe_path,
            executable_path,
            observed_at,
        ],
    )?;
    Ok(())
}

fn insert_session(
    conn: &Connection,
    session_id: &str,
    device: &DeviceProfile,
    observed_at: &str,
    sample_ms: i64,
    app: Option<&AppUsageTarget>,
    usage_kind: UsageKind,
) -> rusqlite::Result<()> {
    let (app_id, window_title, process_name, executable_path, normalized_exe_path, pid, foreground_ms, idle_ms) =
        match app {
            Some(app) => (
                Some(app.app_id.clone()),
                app.window_title.clone(),
                Some(app.process_name.clone()),
                app.executable_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                app.normalized_exe_path.clone(),
                Some(app.pid as i64),
                sample_ms,
                0,
            ),
            None => (None, None, None, None, None, None, 0, sample_ms),
        };

    conn.execute(
        r#"
        INSERT INTO usage_sessions (
            id,
            device_id,
            app_id,
            usage_kind,
            started_at,
            last_seen_at,
            ended_at,
            foreground_ms,
            idle_ms,
            sample_count,
            window_title,
            process_name,
            executable_path,
            normalized_exe_path,
            pid_start,
            pid_end,
            metadata_json
        )
        VALUES (
            ?1, ?2, ?3, ?4, ?5, ?5, NULL, ?6, ?7, 1, ?8, ?9, ?10, ?11, ?12, ?12, NULL
        )
        "#,
        params![
            session_id,
            device.device_id,
            app_id,
            usage_kind.as_str(),
            observed_at,
            foreground_ms,
            idle_ms,
            window_title,
            process_name,
            executable_path,
            normalized_exe_path,
            pid,
        ],
    )?;

    Ok(())
}

fn update_session(
    conn: &Connection,
    session_id: &str,
    observed_at: &str,
    sample_ms: i64,
    app: Option<&AppUsageTarget>,
) -> rusqlite::Result<()> {
    let (window_title, process_name, executable_path, normalized_exe_path, pid, foreground_ms, idle_ms) =
        match app {
            Some(app) => (
                app.window_title.clone(),
                Some(app.process_name.clone()),
                app.executable_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                app.normalized_exe_path.clone(),
                Some(app.pid as i64),
                sample_ms,
                0,
            ),
            None => (None, None, None, None, None, 0, sample_ms),
        };

    conn.execute(
        r#"
        UPDATE usage_sessions
        SET
            last_seen_at = ?2,
            foreground_ms = foreground_ms + ?3,
            idle_ms = idle_ms + ?4,
            sample_count = sample_count + 1,
            window_title = ?5,
            process_name = ?6,
            executable_path = ?7,
            normalized_exe_path = ?8,
            pid_end = ?9
        WHERE id = ?1
        "#,
        params![
            session_id,
            observed_at,
            foreground_ms,
            idle_ms,
            window_title,
            process_name,
            executable_path,
            normalized_exe_path,
            pid,
        ],
    )?;
    Ok(())
}

fn close_session(conn: &Connection, session_id: &str, observed_at: &str) -> rusqlite::Result<()> {
    conn.execute(
        r#"
        UPDATE usage_sessions
        SET ended_at = ?2
        WHERE id = ?1
        "#,
        params![session_id, observed_at],
    )?;
    Ok(())
}

fn append_event(
    conn: &Connection,
    state: &mut TrackerState,
    observed_at: &str,
    device: &DeviceProfile,
    session_id: &str,
    sample_ms: i64,
    app: Option<&AppUsageTarget>,
    usage_kind: UsageKind,
    event_kind: &str,
) -> rusqlite::Result<()> {
    let event_id = next_event_id(state, observed_at);
    let (app_id, window_title, process_name, executable_path, normalized_exe_path, pid, is_foreground, is_idle, foreground_ms, idle_ms) =
        match app {
            Some(app) => (
                Some(app.app_id.clone()),
                app.window_title.clone(),
                Some(app.process_name.clone()),
                app.executable_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                app.normalized_exe_path.clone(),
                Some(app.pid as i64),
                1,
                0,
                sample_ms,
                0,
            ),
            None => (None, None, None, None, None, None, 0, 1, 0, sample_ms),
        };

    conn.execute(
        r#"
        INSERT INTO usage_events (
            id,
            device_id,
            app_id,
            session_id,
            occurred_at,
            usage_kind,
            event_kind,
            sample_ms,
            foreground_ms,
            idle_ms,
            window_title,
            process_name,
            executable_path,
            normalized_exe_path,
            pid,
            is_foreground,
            is_idle,
            metadata_json
        )
        VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, NULL
        )
        "#,
        params![
            event_id,
            device.device_id,
            app_id,
            session_id,
            observed_at,
            usage_kind.as_str(),
            event_kind,
            sample_ms,
            foreground_ms,
            idle_ms,
            window_title,
            process_name,
            executable_path,
            normalized_exe_path,
            pid,
            is_foreground,
            is_idle,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::UsageTracker;
    use crate::db::UsageDb;
    use crate::model::{AppUsageTarget, DeviceProfile, UsageSnapshot, PLATFORM_WINDOWS};
    use chrono::{Duration as ChronoDuration, TimeZone, Utc};
    use tempfile::NamedTempFile;
    use std::time::Duration;

    fn setup_tracker() -> (NamedTempFile, UsageTracker) {
        let file = NamedTempFile::new().expect("temp db");
        let db = UsageDb::new(file.path());
        db.initialize_schema().expect("schema");

        (file, UsageTracker::new(db))
    }

    #[test]
    fn records_sessions_and_events_for_foreground_and_idle_samples() {
        let (_file, mut tracker) = setup_tracker();
        let device = DeviceProfile::new("device-1", "Device One", PLATFORM_WINDOWS);

        let t0 = Utc.with_ymd_and_hms(2026, 4, 16, 10, 0, 0).unwrap();
        let app = AppUsageTarget::from_process(
            Some(std::path::Path::new(r"C:\Apps\Example.exe")),
            "Example",
            Some("Example Window".to_string()),
            321,
        );

        tracker
            .process_snapshot(
                UsageSnapshot::foreground(t0, device.clone(), app.clone()),
                Duration::from_secs(10),
            )
            .expect("first foreground sample");

        tracker
            .process_snapshot(
                UsageSnapshot::foreground(
                    t0 + ChronoDuration::seconds(10),
                    device.clone(),
                    app,
                ),
                Duration::from_secs(10),
            )
            .expect("second foreground sample");

        tracker
            .process_snapshot(
                UsageSnapshot::idle(t0 + ChronoDuration::seconds(20), device, 125_000),
                Duration::from_secs(10),
            )
            .expect("idle sample");

        let counts = tracker
            .db
            .with_conn(|conn| {
                let device_count: i64 =
                    conn.query_row("SELECT COUNT(*) FROM usage_devices", [], |row| row.get(0))?;
                let app_count: i64 =
                    conn.query_row("SELECT COUNT(*) FROM usage_apps", [], |row| row.get(0))?;
                let session_count: i64 =
                    conn.query_row("SELECT COUNT(*) FROM usage_sessions", [], |row| row.get(0))?;
                let event_count: i64 =
                    conn.query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))?;
                let foreground_ms: i64 = conn.query_row(
                    "SELECT foreground_ms FROM usage_sessions WHERE usage_kind = 'foreground' ORDER BY started_at LIMIT 1",
                    [],
                    |row| row.get(0),
                )?;
                let idle_ms: i64 = conn.query_row(
                    "SELECT idle_ms FROM usage_sessions WHERE usage_kind = 'idle' ORDER BY started_at LIMIT 1",
                    [],
                    |row| row.get(0),
                )?;

                Ok((device_count, app_count, session_count, event_count, foreground_ms, idle_ms))
            })
            .expect("counts");

        assert_eq!(counts.0, 1);
        assert_eq!(counts.1, 1);
        assert_eq!(counts.2, 2);
        assert_eq!(counts.3, 3);
        assert_eq!(counts.4, 20_000);
        assert_eq!(counts.5, 10_000);
    }
}
