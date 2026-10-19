use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::Value;

use super::SqliteStorageBackend;
use crate::db::{
    list_object_links, list_object_types, list_objects, load_all_projects, load_all_tags,
    load_all_todos, load_all_tracked_apps, load_usage_event, load_usage_session,
};
use crate::hlc::HLC;
use crate::types::{SyncEntity, VersionVector};

fn load_usage_sequence_page(
    conn: &Connection,
    remote_vector: &VersionVector,
    offset: usize,
    limit: usize,
) -> Result<Vec<SyncEntity>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let mut cursors = remote_vector
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix("@usage:")
                .and_then(|device_id| value.parse::<i64>().ok().map(|seq| (device_id, seq)))
        })
        .collect::<Vec<_>>();
    cursors.sort_unstable_by(|a, b| a.0.cmp(b.0));

    let mut sql = String::from(
        "SELECT device_id, seq, entity_type, entity_id
         FROM usage_sync_log WHERE seq > ",
    );
    let mut values = Vec::<rusqlite::types::Value>::new();
    if cursors.is_empty() {
        sql.push('0');
    } else {
        sql.push_str("CASE device_id ");
        for (device_id, seq) in cursors {
            sql.push_str("WHEN ? THEN ? ");
            values.push(device_id.to_string().into());
            values.push(seq.into());
        }
        sql.push_str("ELSE 0 END");
    }
    sql.push_str(" ORDER BY device_id, seq LIMIT ? OFFSET ?");
    values.push((limit as i64).into());
    values.push((offset as i64).into());

    let mut statement = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let refs = statement
        .query_map(params_from_iter(values), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(statement);

    fn to_data_map<T: serde::Serialize>(item: &T) -> serde_json::Map<String, Value> {
        match serde_json::to_value(item) {
            Ok(Value::Object(mut map)) => {
                map.remove("id");
                map
            }
            _ => serde_json::Map::new(),
        }
    }

    let mut entities = Vec::with_capacity(refs.len());
    for (origin_device_id, seq, entity_type, entity_id) in refs {
        let hlc = conn
            .query_row(
                "SELECT hlc FROM usage_sync_versions
                 WHERE entity_type = ?1 AND entity_id = ?2",
                params![entity_type, entity_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| HLC::now(&origin_device_id).to_string());
        let deleted = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sync_tombstones
                    WHERE entity_type = ?1 AND id = ?2
                 )",
                params![entity_type, entity_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            != 0;
        let data = if deleted {
            Some(serde_json::Map::new())
        } else {
            match entity_type.as_str() {
                "usage_session" => load_usage_session(conn, &entity_id)?
                    .as_ref()
                    .map(to_data_map),
                "usage_event" => load_usage_event(conn, &entity_id)?
                    .as_ref()
                    .map(to_data_map),
                "usage_day" => ark_store::usage::load_usage_day(conn, &entity_id)?
                    .as_ref()
                    .map(to_data_map),
                _ => None,
            }
        };
        // A ref whose entity row is gone with no tombstone is a dead ref
        // (e.g. clear_all residue or a delete that bypassed bookkeeping).
        // Skipping it here would make ref positions diverge from the entity
        // offset callers page by — later entities get re-emitted — and it
        // leaves a gap in the receiver's contiguous usage cursor. Emit a
        // tombstone instead: the row is gone, so the entity is deleted.
        let (data, deleted) = match data {
            Some(data) => (data, deleted),
            None => (serde_json::Map::new(), true),
        };
        entities.push(SyncEntity {
            entity_type,
            id: entity_id,
            data,
            hlc,
            deleted: deleted.then_some(true),
            origin_device_id: Some(origin_device_id),
            origin_seq: Some(seq.max(0) as u64),
        });
    }
    Ok(entities)
}

impl SqliteStorageBackend {
    pub(in crate::db) fn collect_entities_blocking(
        conn: &Connection,
        vector: &VersionVector,
        device_id: &str,
    ) -> Result<Vec<SyncEntity>, String> {
        let mut entities = Vec::new();
        let mut offset = 0;
        loop {
            let page = Self::collect_entities_page_blocking(conn, vector, device_id, offset, 100)?;
            if page.is_empty() {
                break;
            }
            offset += page.len();
            entities.extend(page);
        }
        Ok(entities)
    }

    pub(in crate::db) fn collect_entities_page_blocking(
        conn: &Connection,
        remote_vector: &VersionVector,
        device_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let mut statement = conn
            .prepare("SELECT object_id FROM task_merge_bootstraps")
            .map_err(|error| error.to_string())?;
        let task_merge_object_ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .collect::<Result<std::collections::HashSet<_>, _>>()
            .map_err(|error| error.to_string())?;

        fn to_data_map<T: serde::Serialize>(item: &T) -> serde_json::Map<String, Value> {
            match serde_json::to_value(item) {
                Ok(Value::Object(mut map)) => {
                    map.remove("id");
                    map
                }
                _ => serde_json::Map::new(),
            }
        }

        let local_vector = remote_vector;
        let authoritative_object_hlc = |id: &str| {
            let object_revision = conn
                .query_row(
                    "SELECT hlc FROM object_sync_versions WHERE object_id = ?1",
                    [id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .ok()
                .flatten();
            let tombstone_revision = conn
                .query_row(
                    "SELECT hlc FROM sync_tombstones
                     WHERE id = ?1 AND entity_type = 'object'",
                    [id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .ok()
                .flatten();
            [object_revision, tombstone_revision]
                .into_iter()
                .flatten()
                .max_by(|left, right| HLC::compare_str(left, right))
        };
        let make_entity = |entity_type: &str, id: &str, data| SyncEntity {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
            data,
            hlc: local_vector
                .get(id)
                .cloned()
                .or_else(|| {
                    (entity_type == "object")
                        .then(|| authoritative_object_hlc(id))
                        .flatten()
                })
                .unwrap_or_else(|| HLC::now(device_id).to_string()),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };

        let fixed_count: usize = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM todos) +
                    (SELECT COUNT(*) FROM projects) +
                    (SELECT COUNT(*) FROM tags) +
                    (SELECT COUNT(*) FROM tracked_apps) +
                    (SELECT COUNT(*) FROM object_types) +
                    (SELECT COUNT(*) FROM objects AS object
                     WHERE NOT EXISTS (
                         SELECT 1 FROM task_merge_bootstraps AS bootstrap
                         WHERE bootstrap.object_id = object.id
                     )) +
                    (SELECT COUNT(*) FROM object_links) +
                    (SELECT COUNT(*) FROM sync_tombstones AS tombstone
                     WHERE NOT (
                         tombstone.entity_type = 'object' AND EXISTS (
                             SELECT 1 FROM task_merge_bootstraps AS bootstrap
                             WHERE bootstrap.object_id = tombstone.id
                         )
                     ))",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let mut fixed = Vec::new();
        macro_rules! add_entities {
            ($items:expr, $kind:literal) => {
                if let Ok(items) = $items {
                    for item in &items {
                        fixed.push(make_entity($kind, &item.id, to_data_map(item)));
                    }
                }
            };
        }
        if offset < fixed_count {
            add_entities!(load_all_todos(conn), "todo");
            add_entities!(load_all_projects(conn), "project");
            add_entities!(load_all_tags(conn), "tag");
            add_entities!(load_all_tracked_apps(conn), "tracked_app");
            add_entities!(list_object_types(conn), "object_type");
            let objects = list_objects(conn).map(|objects| {
                objects
                    .into_iter()
                    .filter(|object| !task_merge_object_ids.contains(&object.id))
                    .collect::<Vec<_>>()
            });
            add_entities!(objects, "object");
            add_entities!(list_object_links(conn), "object_link");
            if let Ok(tombstones) = ark_store::sync_tombstones::load_all(conn) {
                fixed.extend(tombstones.into_iter().filter(|entity| {
                    entity.entity_type != "object" || !task_merge_object_ids.contains(&entity.id)
                }));
            }
            fixed.sort_by_key(|entity| (entity.entity_type.clone(), entity.id.clone()));
        }

        let mut entities: Vec<_> = fixed.iter().skip(offset).take(limit).cloned().collect();
        if entities.len() == limit {
            return Ok(entities);
        }

        let usage_offset = offset.saturating_sub(fixed_count);
        let remaining = limit - entities.len();
        match load_usage_sequence_page(conn, remote_vector, usage_offset, remaining) {
            Ok(usage) => entities.extend(usage),
            Err(error) => eprintln!("[ark-core] failed to load usage sync page: {error}"),
        }
        Ok(entities)
    }
}
