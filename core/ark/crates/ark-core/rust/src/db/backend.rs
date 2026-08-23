// ---------------------------------------------------------------------------
// Delete helpers (for StorageBackend)
// ---------------------------------------------------------------------------

pub fn delete_area(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM areas WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_tag(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM tags WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// SqliteStorageBackend
//
// Wraps a shared rusqlite::Connection behind `Arc<Mutex<_>>` and implements
// the async `StorageBackend` trait expected by `sync_server::SyncServer`
// and `sync_client::SyncClient`. Blocking rusqlite work is wrapped in
// `tokio::task::spawn_blocking` so it doesn't stall the async runtime.
// ---------------------------------------------------------------------------

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
                "usage_day" => load_usage_day(conn, &entity_id)?.as_ref().map(to_data_map),
                _ => None,
            }
        };
        let Some(data) = data else {
            continue;
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

pub struct SqliteStorageBackend {
    conn: Arc<Mutex<rusqlite::Connection>>,
    device_id: Arc<Mutex<String>>,
}

impl SqliteStorageBackend {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self {
            conn,
            device_id: Arc::new(Mutex::new(String::new())),
        }
    }

    /// Set the device id used to stamp HLCs on entities that don't yet carry
    /// one in the version vector. Matches the TS `DelphiStorage` behaviour
    /// (`HLC.now(deviceId)` when `vector[id]` is missing).
    pub fn set_device_id(&self, device_id: &str) -> Result<(), String> {
        // Poison recovery: device_id — простой String, poison невозможен от
        // sane code path, но защита cheap и согласована с остальными
        // SqliteStorageBackend lock'ами (см. load_entities / apply_entity).
        let mut guard = self.device_id.lock().unwrap_or_else(|e| e.into_inner());
        *guard = device_id.to_string();
        drop(guard);
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        ensure_usage_sequence_migrated(&conn, device_id)
    }

    pub fn device_id(&self) -> String {
        self.device_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn collect_entities_blocking(
        conn: &Connection,
        vector: &VersionVector,
        device_id: &str,
    ) -> Vec<SyncEntity> {
        let mut entities = Vec::new();
        let mut offset = 0;
        loop {
            let page = Self::collect_entities_page_blocking(conn, vector, device_id, offset, 100);
            if page.is_empty() {
                break;
            }
            offset += page.len();
            entities.extend(page);
        }
        entities
    }

    fn collect_entities_page_blocking(
        conn: &Connection,
        remote_vector: &VersionVector,
        device_id: &str,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity> {
        if limit == 0 {
            return Vec::new();
        }

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
        let make_entity = |entity_type: &str, id: &str, data| SyncEntity {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
            data,
            hlc: local_vector
                .get(id)
                .cloned()
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
                    (SELECT COUNT(*) FROM areas) +
                    (SELECT COUNT(*) FROM tags) +
                    (SELECT COUNT(*) FROM headings) +
                    (SELECT COUNT(*) FROM tracked_apps) +
                    (SELECT COUNT(*) FROM object_types) +
                    (SELECT COUNT(*) FROM objects) +
                    (SELECT COUNT(*) FROM object_links) +
                    (SELECT COUNT(*) FROM sync_tombstones)",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
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
            add_entities!(load_all_areas(conn), "area");
            add_entities!(load_all_tags(conn), "tag");
            add_entities!(load_all_headings(conn), "heading");
            add_entities!(load_all_tracked_apps(conn), "tracked_app");
            add_entities!(list_object_types(conn), "object_type");
            add_entities!(list_objects(conn), "object");
            add_entities!(list_object_links(conn), "object_link");
            if let Ok(tombstones) = load_sync_tombstones(conn) {
                fixed.extend(tombstones);
            }
            fixed.sort_by(|a, b| {
                a.entity_type
                    .cmp(&b.entity_type)
                    .then_with(|| a.id.cmp(&b.id))
            });
        }

        let mut entities: Vec<_> = fixed.iter().skip(offset).take(limit).cloned().collect();
        if entities.len() == limit {
            return entities;
        }

        let usage_offset = offset.saturating_sub(fixed_count);
        let remaining = limit - entities.len();
        match load_usage_sequence_page(conn, remote_vector, usage_offset, remaining) {
            Ok(usage) => entities.extend(usage),
            Err(error) => eprintln!("[ark-core] failed to load usage sync page: {error}"),
        }
        entities
    }

    fn apply_entity_blocking(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
        conn.execute_batch("SAVEPOINT sync_entity_apply")
            .map_err(|e| e.to_string())?;
        match Self::apply_entity_blocking_inner(conn, entity) {
            Ok(()) => {
                conn.execute_batch("RELEASE sync_entity_apply")
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            Err(error) => {
                let _ =
                    conn.execute_batch("ROLLBACK TO sync_entity_apply; RELEASE sync_entity_apply");
                Err(error)
            }
        }
    }

    fn apply_entity_blocking_inner(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
        let sequenced_usage = is_sequenced_usage_entity(&entity.entity_type);
        if sequenced_usage && !usage_entity_is_newer(conn, entity)? {
            let (device_id, seq) = usage_origin_for_entity(conn, entity)?;
            return record_usage_sequence(
                conn,
                &entity.entity_type,
                &entity.id,
                &device_id,
                seq,
                &entity.hlc,
                entity.deleted == Some(true),
            );
        }
        if matches!(entity.entity_type.as_str(), "area" | "heading") {
            return Err("LegacyPlanningReadOnly".into());
        }
        if matches!(entity.entity_type.as_str(), "todo" | "project" | "tag")
            && entity.deleted == Some(true)
        {
            let mut canonical = entity.clone();
            canonical.entity_type = "object".into();
            return crate::canonical_types::facades::apply_canonical_object_entity(
                conn, &canonical,
            );
        }

        if entity.deleted == Some(true) {
            match entity.entity_type.as_str() {
                "todo" => delete_todo(conn, &entity.id),
                "project" => delete_project(conn, &entity.id),
                "area" => delete_area(conn, &entity.id),
                "tag" => delete_tag(conn, &entity.id),
                "heading" => delete_heading(conn, &entity.id),
                "tracked_app" => delete_tracked_app(conn, &entity.id),
                "usage_session" => delete_usage_session(conn, &entity.id),
                "usage_event" => delete_usage_event(conn, &entity.id),
                "usage_day" => delete_usage_day(conn, &entity.id),
                "object_type" => delete_object_type(conn, &entity.id),
                "object" => delete_object(conn, &entity.id),
                "object_link" => delete_object_link(conn, &entity.id),
                _ => Err(format!("unknown sync entity type '{}'", entity.entity_type)),
            }?;
            upsert_sync_tombstone(conn, entity)?;
            if sequenced_usage {
                finish_usage_entity_sync(conn, entity)?;
            }
            return Ok(());
        }

        let mut full_data = entity.data.clone();
        full_data.insert("id".to_string(), Value::String(entity.id.clone()));
        let value = Value::Object(full_data);

        let result = match entity.entity_type.as_str() {
            "todo" | "project" | "tag" => {
                crate::canonical_types::facades::apply_legacy_compat_entity(conn, entity)
            }
            "area" | "heading" => Err("LegacyPlanningReadOnly".into()),
            "tracked_app" => serde_json::from_value::<TrackedApp>(value)
                .map_err(|e| e.to_string())
                .and_then(|tracked_app| upsert_tracked_app(conn, &tracked_app)),
            "usage_session" => serde_json::from_value::<UsageSession>(value)
                .map_err(|e| e.to_string())
                .and_then(|session| upsert_usage_session(conn, &session)),
            "usage_event" => serde_json::from_value::<UsageEvent>(value)
                .map_err(|e| e.to_string())
                .and_then(|event| upsert_usage_event(conn, &event)),
            "usage_day" => serde_json::from_value::<UsageDay>(value)
                .map_err(|e| e.to_string())
                .and_then(|day| upsert_usage_day(conn, &day)),
            "object_type" => serde_json::from_value::<ObjectType>(value)
                .map_err(|e| e.to_string())
                .and_then(|object_type| upsert_object_type(conn, &object_type)),
            "object" => {
                // The canonical ingress owns identity resolution and validation;
                // pending replay uses this same branch after the registry arrives.
                let mut object_data = match value.clone() {
                    Value::Object(map) => map,
                    _ => return Err("object payload must be an object".into()),
                };
                object_data
                    .entry("typeVersion".to_string())
                    .or_insert_with(|| Value::String("0.0.0-legacy".to_string()));
                let object = serde_json::from_value::<ArkObject>(Value::Object(object_data))
                    .map_err(|e| e.to_string())?;
                if !is_object_definition_known(conn, &object.type_id, &object.type_version)? {
                    insert_pending_object(conn, entity, &object.type_id)?;
                    crate::events::emit_event(json!({
                        "event": "sync_error",
                        "code": "unknown_type_version",
                        "entity_type": "object",
                        "entity_id": entity.id,
                        "awaited_type_id": object.type_id,
                        "awaited_type_version": object.type_version,
                    }));
                    Ok(())
                } else {
                    crate::canonical_types::facades::apply_canonical_object_entity(conn, entity)
                }
            }
            "object_link" => serde_json::from_value::<ObjectLink>(value)
                .map_err(|e| e.to_string())
                .and_then(|link| upsert_object_link(conn, &link)),
            _ => Err(format!("unknown sync entity type '{}'", entity.entity_type)),
        };
        result?;
        delete_sync_tombstone(conn, &entity.id)?;
        if sequenced_usage {
            finish_usage_entity_sync(conn, entity)?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl StorageBackend for SqliteStorageBackend {
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity> {
        let conn = self.conn.clone();
        let vector = vector.clone();
        let device_id = self.device_id();
        tokio::task::spawn_blocking(move || {
            // Poison recovery: если earlier panic заполучил lock, мы всё
            // равно можем читать. Это backend для load (read-only path),
            // данные внутри guard'а целы. Без recovery каждый последующий
            // sync round возвращает empty list → multi-device sync silent
            // фейлится навсегда после первого panic'а в этом процессе.
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            Self::collect_entities_blocking(&guard, &vector, &device_id)
        })
        .await
        .unwrap_or_default()
    }

    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity> {
        let conn = self.conn.clone();
        let vector = vector.clone();
        let device_id = self.device_id();
        tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            Self::collect_entities_page_blocking(&guard, &vector, &device_id, offset, limit)
        })
        .await
        .unwrap_or_default()
    }

    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String> {
        let conn = self.conn.clone();
        let entity = entity.clone();
        tokio::task::spawn_blocking(move || {
            // Apply — write path. Poison recovery acceptable: SQLite
            // transactions atomic, partially-applied state не возможен.
            // Альтернатива (return Err) делает sync неработоспособным до
            // process restart.
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            Self::apply_entity_blocking(&guard, &entity)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    async fn get_kv(&self, key: &str) -> Option<String> {
        let conn = self.conn.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            get_sync_kv(&guard, &key).unwrap_or(None)
        })
        .await
        .unwrap_or(None)
    }

    async fn set_kv(&self, key: &str, value: &str) {
        let conn = self.conn.clone();
        let key = key.to_string();
        let value = value.to_string();
        let _ = tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            let _ = set_sync_kv(&guard, &key, &value);
        })
        .await;
    }
}

// ---------------------------------------------------------------------------
