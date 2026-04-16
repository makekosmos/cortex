use crate::database::{with_db, with_db_mut};
use chrono::Utc;
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogueItem {
    pub id: i64,
    pub rawg_id: i64,
    pub name: String,
    pub payload: String,
    pub source: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpsertCatalogueItem {
    pub id: Option<i64>,
    pub rawg_id: i64,
    pub name: String,
    pub payload: String,
    pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub synced: usize,
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn map_row(row: &Row) -> rusqlite::Result<CatalogueItem> {
    Ok(CatalogueItem {
        id: row.get(0)?,
        rawg_id: row.get(1)?,
        name: row.get(2)?,
        payload: row.get(3)?,
        source: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

#[tauri::command]
pub fn get_catalogue_items(source: Option<String>) -> Result<Vec<CatalogueItem>, String> {
    with_db(|conn| {
        let items = if let Some(source) = source {
            let mut stmt = conn.prepare(
                "SELECT id, rawg_id, name, payload, source, updated_at
                 FROM catalogue_items
                 WHERE source = ?1
                 ORDER BY name ASC",
            )?;
            let rows = stmt.query_map(params![source], map_row)?;
            rows.filter_map(|row| row.ok()).collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, rawg_id, name, payload, source, updated_at
                 FROM catalogue_items
                 ORDER BY name ASC",
            )?;
            let rows = stmt.query_map([], map_row)?;
            rows.filter_map(|row| row.ok()).collect()
        };

        Ok(items)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_catalogue(query: String, source: Option<String>) -> Result<Vec<CatalogueItem>, String> {
    let trimmed = query.trim().to_string();
    if trimmed.is_empty() {
        return get_catalogue_items(source);
    }

    let pattern = format!("%{trimmed}%");

    with_db(|conn| {
        if let Some(source) = source {
            let mut stmt = conn.prepare(
                "SELECT id, rawg_id, name, payload, source, updated_at
                 FROM catalogue_items
                 WHERE source = ?1 AND (name LIKE ?2 OR payload LIKE ?2)
                 ORDER BY name ASC",
            )?;
            let rows = stmt
                .query_map(params![source, pattern], map_row)?;
            Ok(rows
                .filter_map(|row| row.ok())
                .collect())
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, rawg_id, name, payload, source, updated_at
                 FROM catalogue_items
                 WHERE name LIKE ?1 OR payload LIKE ?1
                 ORDER BY name ASC",
            )?;
            let rows = stmt
                .query_map(params![pattern], map_row)?;
            Ok(rows
                .filter_map(|row| row.ok())
                .collect())
        }
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn upsert_catalogue_item(input: UpsertCatalogueItem) -> Result<CatalogueItem, String> {
    if input.name.trim().is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    if serde_json::from_str::<Value>(&input.payload).is_err() {
        return Err("Payload must be valid JSON".to_string());
    }

    let source = input.source.unwrap_or_else(|| "rawg".to_string()).trim().to_string();
    let now = now();

    if let Some(id) = input.id {
        with_db(|conn| {
            let updated = conn.execute(
                "UPDATE catalogue_items
                 SET rawg_id = ?1, name = ?2, payload = ?3, source = ?4, updated_at = ?5
                 WHERE id = ?6",
                params![input.rawg_id, input.name, input.payload, source, now, id],
            )?;
            if updated == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }

            let mut stmt = conn.prepare(
                "SELECT id, rawg_id, name, payload, source, updated_at
                 FROM catalogue_items WHERE id = ?1",
            )?;
            stmt.query_row(params![id], map_row)
        })
        .map_err(|e| e.to_string())
    } else {
        with_db(|conn| {
            conn.execute(
                "INSERT INTO catalogue_items (rawg_id, name, payload, source, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(rawg_id, source) DO UPDATE
                 SET name = excluded.name,
                     payload = excluded.payload,
                     updated_at = excluded.updated_at",
                params![input.rawg_id, input.name, input.payload, source, now],
            )?;

            let mut stmt = conn.prepare(
                "SELECT id, rawg_id, name, payload, source, updated_at
                 FROM catalogue_items WHERE rawg_id = ?1 AND source = ?2 LIMIT 1",
            )?;
            stmt.query_row(params![input.rawg_id, source], map_row)
        })
        .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn delete_catalogue_item(id: i64) -> Result<bool, String> {
    let removed = with_db(|conn| {
        let deleted = conn.execute("DELETE FROM catalogue_items WHERE id = ?1", params![id])?;
        Ok(deleted > 0)
    })
    .map_err(|e| e.to_string())?;

    Ok(removed)
}

#[tauri::command]
pub fn sync_library_to_catalogue() -> Result<SyncResult, String> {
    let items = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT id, name, exe_name FROM games")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .filter_map(|row| row.ok())
            .collect::<Vec<_>>();

        Ok(rows)
    })
    .map_err(|e| e.to_string())?;

    let now = now();
    let source = "library".to_string();

    with_db_mut(|conn| {
        let tx = conn.transaction()?;
        let mut synced = 0usize;

        for (game_id, name, exe_name) in items {
            let rawg_id = stable_hash(&game_id);
            let payload = json!({
                "game_id": game_id,
                "exe_name": exe_name,
            })
            .to_string();

            tx.execute(
                "INSERT INTO catalogue_items (rawg_id, name, payload, source, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(rawg_id, source) DO UPDATE
                 SET name = excluded.name,
                     payload = excluded.payload,
                     updated_at = excluded.updated_at",
                params![rawg_id, name, payload, &source, now],
            )?;

            synced += 1;
        }

        tx.commit()?;
        Ok(SyncResult { synced })
    })
    .map_err(|e| e.to_string())
}

fn stable_hash(value: &str) -> i64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);

    let hashed = hasher.finish() % (i64::MAX as u64);
    (hashed as i64) + 1
}
