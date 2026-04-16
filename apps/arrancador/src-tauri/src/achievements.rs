use crate::database::with_db_mut;
use chrono::Utc;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    pub id: String,
    pub title: String,
    pub description: String,
    pub event_trigger: String,
    pub progress: i64,
    pub target: i64,
    pub unlocked: bool,
    pub unlocked_at: Option<String>,
    pub created_at: String,
}

struct AchievementSeed {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    event_trigger: &'static str,
    target: i64,
}

const DEFAULT_ACHIEVEMENTS: &[AchievementSeed] = &[
    AchievementSeed {
        id: "first-launch",
        title: "First Launch",
        description: "Launch any game once.",
        event_trigger: "game_launch",
        target: 1,
    },
    AchievementSeed {
        id: "backup-guardian",
        title: "Backup Guardian",
        description: "Complete a backup or restore flow.",
        event_trigger: "backup_restore",
        target: 1,
    },
    AchievementSeed {
        id: "download-scout",
        title: "Download Scout",
        description: "Register a completed download event.",
        event_trigger: "download_complete",
        target: 1,
    },
    AchievementSeed {
        id: "scan-master",
        title: "Scan Master",
        description: "Finish a library scan.",
        event_trigger: "scan_complete",
        target: 1,
    },
];

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn map_row(row: &rusqlite::Row) -> Result<Achievement> {
    Ok(Achievement {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        event_trigger: row.get(3)?,
        progress: row.get(4)?,
        target: row.get(5)?,
        unlocked: row.get::<_, i32>(6)? == 1,
        unlocked_at: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn seed_defaults(conn: &mut Connection) -> Result<()> {
    let created_at = now();
    let tx = conn.transaction()?;

    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO achievements
                (id, title, description, event_trigger, progress, target, unlocked, unlocked_at, created_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, 0, NULL, ?6)",
        )?;

        for seed in DEFAULT_ACHIEVEMENTS {
            stmt.execute(params![
                seed.id,
                seed.title,
                seed.description,
                seed.event_trigger,
                seed.target,
                created_at,
            ])?;
        }
    }

    tx.commit()?;
    Ok(())
}

fn fetch_achievements(conn: &Connection, unlocked_only: bool) -> Result<Vec<Achievement>> {
    let mut stmt = if unlocked_only {
        conn.prepare(
            "SELECT id, title, description, event_trigger, progress, target, unlocked, unlocked_at, created_at
             FROM achievements
             WHERE unlocked = 1
             ORDER BY COALESCE(unlocked_at, created_at) DESC, title ASC",
        )?
    } else {
        conn.prepare(
            "SELECT id, title, description, event_trigger, progress, target, unlocked, unlocked_at, created_at
             FROM achievements
             ORDER BY unlocked DESC, COALESCE(unlocked_at, created_at) DESC, title ASC",
        )?
    };

    let rows = stmt
        .query_map([], map_row)?
        .filter_map(|row| row.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn get_all_achievements(unlocked_only: bool) -> Result<Vec<Achievement>, String> {
    with_db_mut(|conn| {
        seed_defaults(conn)?;
        fetch_achievements(conn, unlocked_only)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn seed_default_achievements() -> Result<Vec<Achievement>, String> {
    with_db_mut(|conn| {
        seed_defaults(conn)?;
        fetch_achievements(conn, false)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn record_achievement_event(
    event_trigger: String,
    event_context: Option<String>,
) -> Result<Vec<Achievement>, String> {
    let trigger = event_trigger.trim().to_string();
    let _ = event_context;

    with_db_mut(|conn| {
        seed_defaults(conn)?;

        if trigger.is_empty() {
            return fetch_achievements(conn, false);
        }

        let unlocked_at = now();
        conn.execute(
            "UPDATE achievements
             SET progress = CASE WHEN progress < target THEN progress + 1 ELSE progress END,
                 unlocked = CASE WHEN progress + 1 >= target THEN 1 ELSE unlocked END,
                 unlocked_at = CASE
                     WHEN unlocked = 0 AND progress + 1 >= target THEN ?1
                     ELSE unlocked_at
                 END
             WHERE event_trigger = ?2",
            params![unlocked_at, trigger],
        )?;

        fetch_achievements(conn, false)
    })
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{init_schema, set_test_db, TEST_DB_MUTEX};
    use rusqlite::Connection;

    #[test]
    fn seeds_default_achievements_once() {
        let _guard = TEST_DB_MUTEX.lock().unwrap();
        let conn = Connection::open_in_memory().expect("open in-memory db");
        init_schema(&conn).expect("init schema");
        let _db = set_test_db(conn);

        let seeded = seed_default_achievements().expect("seed defaults");
        assert_eq!(seeded.len(), DEFAULT_ACHIEVEMENTS.len());
        assert!(seeded.iter().any(|item| item.id == "first-launch"));
    }

    #[test]
    fn records_matching_event_and_unlocks() {
        let _guard = TEST_DB_MUTEX.lock().unwrap();
        let conn = Connection::open_in_memory().expect("open in-memory db");
        init_schema(&conn).expect("init schema");
        let _db = set_test_db(conn);

        let updated = record_achievement_event("game_launch".to_string(), None)
            .expect("record event");
        let launch = updated
            .iter()
            .find(|item| item.event_trigger == "game_launch")
            .expect("launch achievement");

        assert!(launch.unlocked);
        assert_eq!(launch.progress, launch.target);
        assert!(launch.unlocked_at.is_some());
    }
}
