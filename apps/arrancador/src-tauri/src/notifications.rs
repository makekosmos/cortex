use crate::database::with_db;
use chrono::Utc;
use rusqlite::{params, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationItem {
    pub id: String,
    pub level: String,
    pub title: String,
    pub message: String,
    pub source: Option<String>,
    pub created_at: String,
    pub read_at: Option<String>,
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn map_row(row: &rusqlite::Row) -> Result<NotificationItem> {
    Ok(NotificationItem {
        id: row.get(0)?,
        level: row.get(1)?,
        title: row.get(2)?,
        message: row.get(3)?,
        source: row.get(4)?,
        created_at: row.get(5)?,
        read_at: row.get(6)?,
    })
}

fn create_internal(level: &str, title: &str, message: &str, source: Option<&str>) -> Result<NotificationItem> {
    let id = Uuid::new_v4().to_string();
    let created_at = now();

    with_db(|conn| {
        conn.execute(
            "INSERT INTO notifications (id, level, title, message, source, created_at, read_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![id, level, title, message, source, created_at],
        )?;

        Ok(NotificationItem {
            id,
            level: level.to_string(),
            title: title.to_string(),
            message: message.to_string(),
            source: source.map(|value| value.to_string()),
            created_at,
            read_at: None,
        })
    })
    .map_err(|e| {
        eprintln!("Failed to create notification: {e}");
        e
    })
}

#[tauri::command]
pub fn list_notifications(unread_only: bool) -> Result<Vec<NotificationItem>, String> {
    with_db(|conn| {
        let mut stmt = if unread_only {
            conn.prepare(
                "SELECT id, level, title, message, source, created_at, read_at
                 FROM notifications
                 WHERE read_at IS NULL
                 ORDER BY created_at DESC
                 LIMIT 200",
            )?
        } else {
            conn.prepare(
                "SELECT id, level, title, message, source, created_at, read_at
                 FROM notifications
                 ORDER BY created_at DESC
                 LIMIT 200",
            )?
        };

        let rows = stmt.query_map([], map_row)?.filter_map(|row| row.ok()).collect();
        Ok(rows)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_notification(
    level: String,
    title: String,
    message: String,
    source: Option<String>,
) -> Result<NotificationItem, String> {
    create_internal(&level, &title, &message, source.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mark_notification_read(id: String) -> Result<bool, String> {
    with_db(|conn| {
        let now = now();
        let updated = conn.execute(
            "UPDATE notifications SET read_at = ?1 WHERE id = ?2 AND read_at IS NULL",
            params![now, id],
        )?;
        Ok(updated > 0)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mark_all_notifications_read() -> Result<u64, String> {
    with_db(|conn| {
        let now = now();
        let updated = conn.execute(
            "UPDATE notifications SET read_at = ?1 WHERE read_at IS NULL",
            params![now],
        )?;
        Ok(u64::try_from(updated).unwrap_or(0))
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_notifications() -> Result<usize, String> {
    with_db(|conn| {
        let count: usize = conn
            .query_row("SELECT COUNT(*) FROM notifications", [], |row| row.get(0))
            .unwrap_or(0);
        conn.execute("DELETE FROM notifications", [])?;
        Ok(count)
    })
    .map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn notify_success(title: &str, message: &str) {
    let _ = create_internal("success", title, message, None::<&str>);
}

#[allow(dead_code)]
pub fn notify_info(title: &str, message: &str) {
    let _ = create_internal("info", title, message, None::<&str>);
}

#[allow(dead_code)]
pub fn notify_warning(title: &str, message: &str) {
    let _ = create_internal("warning", title, message, None::<&str>);
}

#[allow(dead_code)]
pub fn notify_error(title: &str, message: &str) {
    let _ = create_internal("error", title, message, None::<&str>);
}
