use rusqlite::{params, Connection, Result};
use std::path::PathBuf;
use std::sync::Mutex;

lazy_static::lazy_static! {
    pub static ref DB: Mutex<Option<Connection>> = Mutex::new(None);
}

#[cfg(test)]
lazy_static::lazy_static! {
    pub(crate) static ref TEST_DB_MUTEX: Mutex<()> = Mutex::new(());
}

#[cfg(test)]
pub(crate) struct TestDbGuard;

#[cfg(test)]
impl Drop for TestDbGuard {
    fn drop(&mut self) {
        let mut db = DB.lock().unwrap();
        *db = None;
    }
}

#[cfg(test)]
pub(crate) fn set_test_db(conn: Connection) -> TestDbGuard {
    let mut db = DB.lock().unwrap();
    *db = Some(conn);
    TestDbGuard
}

fn enable_foreign_keys(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(())
}

pub fn get_db_path() -> PathBuf {
    let app_data = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    let db_dir = app_data.join("arrancador");
    std::fs::create_dir_all(&db_dir).ok();
    db_dir.join("arrancador.db")
}

pub fn init_database() -> Result<()> {
    let db_path = get_db_path();
    println!("Initializing database at: {:?}", db_path);

    let conn = Connection::open(&db_path)?;
    enable_foreign_keys(&conn)?;

    init_schema(&conn)?;

    let mut db = DB.lock().unwrap();
    *db = Some(conn);

    println!("Database initialized successfully");
    Ok(())
}

pub(crate) fn init_schema(conn: &Connection) -> Result<()> {
    // Games table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS games (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            exe_path TEXT NOT NULL UNIQUE,
            exe_name TEXT NOT NULL,

            -- Metadata from RAWG
            rawg_id INTEGER,
            description TEXT,
            released TEXT,
            background_image TEXT,
            metacritic INTEGER,
            rating REAL,
            genres TEXT,
            platforms TEXT,
            developers TEXT,
            publishers TEXT,

            -- Local metadata
            cover_image TEXT,
            icon_image TEXT,
            is_favorite INTEGER DEFAULT 0,
            play_count INTEGER DEFAULT 0,
            total_playtime INTEGER DEFAULT 0,
            last_played TEXT,
            date_added TEXT NOT NULL,

            -- Backup settings
            backup_enabled INTEGER DEFAULT 0,
            last_backup TEXT,
            backup_count INTEGER DEFAULT 0,
            save_path TEXT,
            save_path_checked INTEGER DEFAULT 0,

            -- User rating
            user_rating INTEGER,
            user_note TEXT,

            -- Play progress
            play_status TEXT NOT NULL DEFAULT 'not_started'
        )",
        [],
    )?;

    ensure_game_columns(conn)?;
    ensure_game_indexes(conn)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS playtime_daily (
            game_id TEXT NOT NULL,
            date TEXT NOT NULL,
            seconds INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE,
            UNIQUE(game_id, date)
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_playtime_daily_date ON playtime_daily(date)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_playtime_daily_game ON playtime_daily(game_id)",
        [],
    )?;

    // Backups table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS backups (
            id TEXT PRIMARY KEY,
            game_id TEXT NOT NULL,
            backup_path TEXT NOT NULL,
            backup_size INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            is_auto INTEGER DEFAULT 0,
            notes TEXT,
            FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE
        )",
        [],
    )?;
    ensure_backup_indexes(conn)?;

    // Settings table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;

    // Scan history table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS scan_directories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            last_scanned TEXT,
            auto_scan INTEGER DEFAULT 0
        )",
        [],
    )?;

    // Notifications
    conn.execute(
        "CREATE TABLE IF NOT EXISTS notifications (
            id TEXT PRIMARY KEY,
            level TEXT NOT NULL,
            title TEXT NOT NULL,
            message TEXT NOT NULL,
            source TEXT,
            created_at TEXT NOT NULL,
            read_at TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_notifications_created_at ON notifications(created_at DESC)",
        [],
    )?;

    // Achievements
    conn.execute(
        "CREATE TABLE IF NOT EXISTS achievements (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            event_trigger TEXT NOT NULL,
            progress INTEGER NOT NULL DEFAULT 0,
            target INTEGER NOT NULL,
            unlocked INTEGER NOT NULL DEFAULT 0,
            unlocked_at TEXT,
            created_at TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_achievements_trigger ON achievements(event_trigger)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_achievements_unlocked ON achievements(unlocked, unlocked_at DESC)",
        [],
    )?;

    // Catalogue
    conn.execute(
        "CREATE TABLE IF NOT EXISTS catalogue_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            rawg_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            payload TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'rawg',
            updated_at TEXT NOT NULL,
            UNIQUE(rawg_id, source)
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_catalogue_items_rawg_id ON catalogue_items(rawg_id)",
        [],
    )?;

    // Initialize default settings
    let default_settings = vec![
        ("ludusavi_path", ""),
        ("backup_directory", ""),
        ("auto_backup", "true"),
        ("backup_before_launch", "false"),
        ("backup_compression_enabled", "true"),
        ("backup_compression_level", "60"),
        ("backup_skip_compression_once", "false"),
        ("max_backups_per_game", "5"),
        ("theme", "system"),
        ("start_minimized_in_tray", "false"),
    ];

    for (key, value) in default_settings {
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
    }

    Ok(())
}

fn ensure_game_columns(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(games)")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let mut cols = std::collections::HashSet::new();
    for name in rows.flatten() {
        cols.insert(name);
    }

    if !cols.contains("user_rating") {
        conn.execute("ALTER TABLE games ADD COLUMN user_rating INTEGER", [])?;
    }
    if !cols.contains("user_note") {
        conn.execute("ALTER TABLE games ADD COLUMN user_note TEXT", [])?;
    }
    if !cols.contains("save_path") {
        conn.execute("ALTER TABLE games ADD COLUMN save_path TEXT", [])?;
    }
    if !cols.contains("save_path_checked") {
        conn.execute(
            "ALTER TABLE games ADD COLUMN save_path_checked INTEGER DEFAULT 0",
            [],
        )?;
    }
    if !cols.contains("icon_image") {
        conn.execute("ALTER TABLE games ADD COLUMN icon_image TEXT", [])?;
    }
    if !cols.contains("play_status") {
        conn.execute(
            "ALTER TABLE games ADD COLUMN play_status TEXT NOT NULL DEFAULT 'not_started'",
            [],
        )?;
    }

    Ok(())
}

fn ensure_game_indexes(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_games_name ON games(name)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_games_favorite_name ON games(is_favorite, name)",
        [],
    )?;
    Ok(())
}

fn ensure_backup_indexes(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_backups_game_created ON backups(game_id, created_at DESC)",
        [],
    )?;
    Ok(())
}

pub fn with_db<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&Connection) -> Result<T>,
{
    let db = DB.lock().unwrap();
    let conn = db.as_ref().ok_or(rusqlite::Error::InvalidQuery)?;
    enable_foreign_keys(conn)?;
    f(conn)
}

pub fn with_db_mut<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut Connection) -> Result<T>,
{
    let mut db = DB.lock().unwrap();
    let conn = db.as_mut().ok_or(rusqlite::Error::InvalidQuery)?;
    enable_foreign_keys(conn)?;
    f(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::collections::HashSet;

    #[test]
    fn ensure_game_columns_adds_missing_fields_and_is_idempotent() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute(
            "CREATE TABLE games (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                exe_path TEXT NOT NULL UNIQUE,
                exe_name TEXT NOT NULL,
                date_added TEXT NOT NULL
            )",
            [],
        )
        .expect("create games table");

        ensure_game_columns(&conn).expect("ensure columns");
        ensure_game_columns(&conn).expect("ensure columns second time");

        let mut stmt = conn
            .prepare("PRAGMA table_info(games)")
            .expect("pragma table_info");
        let columns: HashSet<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query columns")
            .flatten()
            .collect();

        for column in [
            "user_rating",
            "user_note",
            "save_path",
            "save_path_checked",
            "icon_image",
            "play_status",
        ] {
            assert!(columns.contains(column));
        }
    }
}
