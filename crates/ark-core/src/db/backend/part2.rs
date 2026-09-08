
pub struct SqliteStorageBackend {
    conn: Arc<Mutex<rusqlite::Connection>>,
    device_id: Arc<Mutex<String>>,
    selective_profile: Arc<Mutex<Option<crate::data_platform::SelectiveSyncProfile>>>,
}

impl SqliteStorageBackend {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self {
            conn,
            device_id: Arc::new(Mutex::new(String::new())),
            selective_profile: Arc::new(Mutex::new(None)),
        }
    }

    /// Set the device id used to stamp HLCs on entities that don't yet carry
    /// one in the version vector. Matches the TS `DelphiStorage` behaviour
    /// (`HLC.now(deviceId)` when `vector[id]` is missing).
    pub fn set_device_id(&self, device_id: &str) -> Result<(), String> {
        // Poison recovery: device_id вЂ” РїСЂРѕСЃС‚РѕР№ String, poison РЅРµРІРѕР·РјРѕР¶РµРЅ РѕС‚
        // sane code path, РЅРѕ Р·Р°С‰РёС‚Р° cheap Рё СЃРѕРіР»Р°СЃРѕРІР°РЅР° СЃ РѕСЃС‚Р°Р»СЊРЅС‹РјРё
        // SqliteStorageBackend lock'Р°РјРё (СЃРј. load_entities / apply_entity).
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
                .or_else(|| (entity_type == "object").then(|| authoritative_object_hlc(id)).flatten())
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
            add_entities!(load_all_tags(conn), "tag");
            add_entities!(load_all_tracked_apps(conn), "tracked_app");
            add_entities!(list_object_types(conn), "object_type");
            add_entities!(list_objects(conn), "object");
            add_entities!(list_object_links(conn), "object_link");
            if let Ok(tombstones) = load_sync_tombstones(conn) {
                fixed.extend(tombstones);
            }
            fixed.sort_by_key(|entity| (entity.entity_type.clone(), entity.id.clone()));
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
        if entity.entity_type == "object" && entity.deleted == Some(true) {
            return crate::canonical_types::facades::apply_canonical_object_entity(conn, entity);
        }

        if entity.deleted == Some(true) {
            match entity.entity_type.as_str() {
                "todo" => delete_todo(conn, &entity.id),
                "project" => delete_project(conn, &entity.id),
                "tag" => delete_tag(conn, &entity.id),
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
