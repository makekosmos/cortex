// SQLite store для app-index.
//
// Отдельная база `<data_dir>/app-index.db`, НЕ ARK. См. spec.md → Architecture
// decisions: app-индекс host-specific и regenerable, не синхронизируется.

use crate::app_index::app::{App, AppKind};
use crate::app_index::Result;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

pub struct AppStore {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS apps (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    exec_path TEXT NOT NULL,
    icon_path TEXT,
    kind TEXT NOT NULL,
    source TEXT NOT NULL,
    mtime INTEGER NOT NULL,
    last_indexed_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS apps_name_ci ON apps(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS apps_source ON apps(source);
"#;

impl AppStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        // Mutex poison recovery (см. concepts/db-resilience.md).
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn list_all(&self) -> Result<Vec<App>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, exec_path, icon_path, kind, source, mtime FROM apps ORDER BY name COLLATE NOCASE ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let kind_str: String = row.get(4)?;
            let kind = parse_kind(&kind_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?;
            Ok(App {
                id: row.get(0)?,
                name: row.get(1)?,
                exec_path: row.get(2)?,
                icon_path: row.get(3)?,
                kind,
                source: row.get(5)?,
                mtime: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Полная замена содержимого таблицы. Использовать после rescan'а.
    /// Атомарно через транзакцию.
    pub fn replace_all(&self, apps: &[App]) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM apps", [])?;
        let now = chrono::Utc::now().timestamp();
        {
            let mut stmt = tx.prepare(
                "INSERT INTO apps (id, name, exec_path, icon_path, kind, source, mtime, last_indexed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )?;
            for a in apps {
                stmt.execute(params![
                    a.id,
                    a.name,
                    a.exec_path,
                    a.icon_path,
                    kind_to_str(&a.kind),
                    a.source,
                    a.mtime,
                    now,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

fn kind_to_str(k: &AppKind) -> &'static str {
    match k {
        AppKind::Win32 => "win32",
        AppKind::Uwp => "uwp",
        AppKind::MacBundle => "mac_bundle",
        AppKind::LinuxDesktop => "linux_desktop",
    }
}

fn parse_kind(s: &str) -> std::result::Result<AppKind, String> {
    match s {
        "win32" => Ok(AppKind::Win32),
        "uwp" => Ok(AppKind::Uwp),
        "mac_bundle" => Ok(AppKind::MacBundle),
        "linux_desktop" => Ok(AppKind::LinuxDesktop),
        other => Err(format!("unknown AppKind: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_index::AppIndexError;
    use tempfile::tempdir;

    #[allow(dead_code)]
    fn _use_error(e: AppIndexError) -> AppIndexError {
        e
    }

    #[test]
    fn round_trip() {
        let dir = tempdir().unwrap();
        let store = AppStore::open(&dir.path().join("app-index.db")).unwrap();
        let apps = vec![
            App {
                id: "a1".into(),
                name: "Notepad".into(),
                exec_path: "C:\\notepad.exe".into(),
                icon_path: None,
                kind: AppKind::Win32,
                source: "start_menu".into(),
                mtime: 100,
            },
            App {
                id: "a2".into(),
                name: "Calculator".into(),
                exec_path: "shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into(),
                icon_path: Some("C:\\cache\\calc.png".into()),
                kind: AppKind::Uwp,
                source: "uwp".into(),
                mtime: 200,
            },
        ];
        store.replace_all(&apps).unwrap();
        let loaded = store.list_all().unwrap();
        assert_eq!(loaded.len(), 2);
        // ORDER BY name COLLATE NOCASE: Calculator first.
        assert_eq!(loaded[0].name, "Calculator");
        assert_eq!(loaded[1].name, "Notepad");
        assert_eq!(loaded[1].kind, AppKind::Win32);
    }
}
