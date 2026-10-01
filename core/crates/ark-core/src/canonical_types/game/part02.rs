
fn links_for(conn: &Connection, id: &str) -> Result<Vec<GameLink>, String> {
    Ok(db::list_object_links(conn)?
        .into_iter()
        .filter(|link| link.source_object_id == id)
        .map(|link| GameLink {
            id: link.id,
            link_type: link.link_type,
            target_object_id: link.target_object_id,
        })
        .collect())
}

fn usage_for(conn: &Connection, id: &str) -> Result<GameUsage, String> {
    // The link pins ONE tracked_app id — the id used to hash the full exe
    // path, so an update across a version dir split the app's sessions under
    // a second id and playtime silently halved (KOS-287). Expand the link to
    // every tracked_app row sharing the canonical exe identity before summing.
    let linked_ids: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT target_object_id FROM object_links
                 WHERE source_object_id=?1 AND link_type IN ('game','game-usage')",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };
    let wanted_ids = usage_session_app_ids(conn, &linked_ids)?;
    if wanted_ids.is_empty() {
        return Ok(GameUsage {
            seconds: 0,
            session_count: 0,
            last_played_at: None,
        });
    }
    let placeholders = wanted_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    // Active play time: foreground && !idle. julianday(ended-started)
    // wall-clock counted idle and never-closed sessions (KOS-287).
    let sql = format!(
        "SELECT COALESCE(SUM(s.foreground_ms) / 1000, 0), COUNT(s.id),
                MAX(COALESCE(s.ended_at, s.started_at))
         FROM usage_sessions s WHERE s.tracked_app_id IN ({placeholders})"
    );
    let row: Option<(i64, i64, Option<String>)> = conn
        .query_row(
            &sql,
            params_from_iter(wanted_ids.iter()),
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (seconds, count, last) = row.unwrap_or((0, 0, None));
    Ok(GameUsage {
        seconds: seconds.max(0),
        session_count: count.max(0) as u64,
        last_played_at: last,
    })
}

/// linked tracked_app ids → every tracked_app row sharing the canonical exe
/// identity (version dirs, path case, update moves). Linked ids missing from
/// tracked_apps are kept verbatim — behavior unchanged for dangling links.
fn usage_session_app_ids(conn: &Connection, linked_ids: &[String]) -> Result<Vec<String>, String> {
    if linked_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare("SELECT id, normalized_exe_path, process_name FROM tracked_apps")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut key_by_id = std::collections::HashMap::new();
    for row in rows {
        let (app_id, path, process_name) = row.map_err(|e| e.to_string())?;
        key_by_id.insert(app_id, db::canonical_app_key(&path, &process_name));
    }
    let wanted_keys: std::collections::HashSet<&str> = linked_ids
        .iter()
        .filter_map(|linked| key_by_id.get(linked.as_str()).map(String::as_str))
        .collect();
    let mut ids: Vec<String> = linked_ids.to_vec();
    for (app_id, key) in &key_by_id {
        if wanted_keys.contains(key.as_str()) && !linked_ids.contains(app_id) {
            ids.push(app_id.clone());
        }
    }
    Ok(ids)
}

fn quarantine_for(conn: &Connection, id: &str) -> Result<GameQuarantine, String> {
    let raw: Option<String> = conn.query_row("SELECT fields_json FROM object_migration_quarantine WHERE object_id=?1 AND contract_version=?2", params![id, GAME_QUARANTINE_CONTRACT], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    let value = raw
        .map(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_else(|| json!({}));
    let provider_refs = value
        .get("providerRefs")
        .or_else(|| value.get("provider_refs"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    let fields = value
        .get("fields")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok(GameQuarantine {
        provider_refs,
        fields,
    })
}

pub fn project_game(
    conn: &Connection,
    object: ArkObject,
    device_id: &str,
) -> Result<GameRecord, String> {
    if object.type_id != GAME_TYPE_ID {
        return Err("wrong_type".into());
    }
    if object.type_version != GAME_VERSION {
        return Err("wrong_version".into());
    }
    let (play_status, user_rating, genres, platforms, released, description, extensions) =
        read_game_props(&object)?;
    Ok(GameRecord {
        id: object.id.clone(),
        title: object.title,
        play_status,
        user_rating,
        genres,
        platforms,
        released,
        description,
        extensions,
        created_at: object.created_at,
        updated_at: object.updated_at,
        deleted_at: object.deleted_at,
        links: links_for(conn, &object.id)?,
        local: local_for(conn, &object.id, device_id)?,
        quarantine: quarantine_for(conn, &object.id)?,
        usage: usage_for(conn, &object.id)?,
    })
}

fn local_for(conn: &Connection, id: &str, device_id: &str) -> Result<GameLocalState, String> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT data_json FROM object_local_state WHERE object_id=?1 AND device_id=?2",
            params![id, device_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(raw
        .map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_default())
}

pub fn list_games(conn: &Connection, device_id: &str) -> Result<Vec<GameRecord>, String> {
    db::list_objects_by_type(conn, GAME_TYPE_ID).and_then(|objects| {
        objects
            .into_iter()
            .map(|o| project_game(conn, o, device_id))
            .collect()
    })
}

pub fn get_game(conn: &Connection, id: &str, device_id: &str) -> Result<GameRecord, String> {
    let object = db::get_object(conn, id)?.ok_or_else(|| "game_not_found".to_string())?;
    project_game(conn, object, device_id)
}

pub fn upsert_game(
    conn: &Connection,
    command: GameUpsertCommand,
    device_id: &str,
) -> Result<GameMutationResult, String> {
    conn.execute_batch("SAVEPOINT canonical_game_upsert")
        .map_err(|e| e.to_string())?;
    let effective_device = command.device_id.as_deref().unwrap_or(device_id).to_owned();
    let result = (|| {
        validate_game_command(conn, &command)?;
        validate_game_links(conn, &command.id, &command.links)?;
        if let Some(existing) = db::get_object(conn, &command.id)? {
            let current = project_game(conn, existing, &effective_device)?;
            let same = current.title == command.title
                && current.play_status == command.play_status
                && current.user_rating == command.user_rating
                && current.genres == command.genres
                && current.platforms == command.platforms
                && current.released == command.released
                && current.description == command.description
                && current.extensions == command.extensions
                && current.created_at == command.created_at
                && current.updated_at == command.updated_at
                && current.deleted_at == command.deleted_at
                && current.links == command.links
                && current.local == command.local
                && current.quarantine == command.quarantine;
            if same {
                return Ok(GameMutationResult {
                    record: current,
                    changed: false,
                });
            }
        }
        let prepared = prepare_object(conn, object_write(&command)).map_err(|e| e.to_string())?;
        db::upsert_object(conn, &prepared)?;
        conn.execute("INSERT INTO object_local_state(object_id,device_id,data_json,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(object_id,device_id) DO UPDATE SET data_json=excluded.data_json,updated_at=excluded.updated_at", params![command.id, effective_device, serde_json::to_string(&command.local).map_err(|e| e.to_string())?, command.updated_at]).map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM object_migration_quarantine WHERE object_id=?1 AND contract_version=?2",
            params![command.id, GAME_QUARANTINE_CONTRACT],
        )
        .map_err(|e| e.to_string())?;
        if !command.quarantine.provider_refs.is_empty() || !command.quarantine.fields.is_empty() {
            let fields = serde_json::to_string(&json!({"providerRefs": command.quarantine.provider_refs, "fields": command.quarantine.fields})).map_err(|e| e.to_string())?;
            let source_hash = format!("{:x}", Sha256::digest(fields.as_bytes()));
            conn.execute("INSERT INTO object_migration_quarantine(object_id,contract_version,source_type_id,fields_json,source_hash,updated_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(object_id,contract_version) DO UPDATE SET fields_json=excluded.fields_json,source_hash=excluded.source_hash,updated_at=excluded.updated_at", params![command.id, GAME_QUARANTINE_CONTRACT, GAME_TYPE_ID, fields, source_hash, command.updated_at]).map_err(|e| e.to_string())?;
        }
        let desired: std::collections::HashSet<String> =
            command.links.iter().map(|l| l.id.clone()).collect();
        for existing in db::list_object_links(conn)?.into_iter().filter(|l| {
            l.source_object_id == command.id
                && ["cover-image", "background-image", "note", "task", "tag"]
                    .contains(&l.link_type.as_str())
        }) {
            if !desired.contains(&existing.id) {
                db::delete_object_link(conn, &existing.id)?;
            }
        }
        for link in &command.links {
            conn.execute("INSERT INTO object_links(id,source_object_id,target_object_id,link_type,created_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET target_object_id=excluded.target_object_id,link_type=excluded.link_type", params![link.id, command.id, link.target_object_id, link.link_type, command.created_at]).map_err(|e| e.to_string())?;
        }
        let hlc =
            db::bump_sync_version_vector(conn, "object", &command.id, &effective_device, false)?;
        conn.execute("INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0", params![command.id, hlc]).map_err(|e| e.to_string())?;
        Ok(GameMutationResult {
            record: get_game(conn, &command.id, &effective_device)?,
            changed: true,
        })
    })();
    match result {
        Ok(record) => {
            conn.execute_batch("RELEASE SAVEPOINT canonical_game_upsert")
                .map_err(|e| e.to_string())?;
            Ok(record)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT canonical_game_upsert; RELEASE SAVEPOINT canonical_game_upsert");
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ObjectLink, TrackedApp, UsageSession};

    fn tracked_app(id: &str, normalized_path: &str) -> TrackedApp {
        TrackedApp {
            id: id.to_string(),
            platform: "windows".to_string(),
            exe_path: normalized_path.to_string(),
            normalized_exe_path: normalized_path.to_string(),
            process_name: "game.exe".to_string(),
            display_name: None,
            publisher: None,
            icon_ref: None,
            first_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
            last_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
        }
    }

    fn session(id: &str, app_id: &str, foreground_ms: i64, idle_ms: i64) -> UsageSession {
        UsageSession {
            id: id.to_string(),
            tracked_app_id: app_id.to_string(),
            device_id: "d".to_string(),
            device_name: "D".to_string(),
            platform: "windows".to_string(),
            started_at: "2026-01-01T00:00:00.000Z".to_string(),
            ended_at: Some("2026-01-01T01:00:00.000Z".to_string()),
            runtime_ms: 3_600_000,
            foreground_ms,
            idle_ms,
            window_title: None,
            process_name: "game.exe".to_string(),
            exe_path: String::new(),
            pid_start: None,
            pid_end: None,
            meta_json: json!({}),
        }
    }

    /// KOS-287 regression: a game link pins one tracked_app id, but updates
    /// across version dirs split the app into two ids. Playtime must merge
    /// them by canonical exe identity — and count active (foreground, not
    /// idle) seconds rather than the julianday wall-clock interval.
    #[test]
    fn game_usage_merges_version_dir_splits_and_counts_active_time() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_schema(&conn).unwrap();
        // object_links has FK to objects(id) on both ends.
        crate::db::upsert_object_type(
            &conn,
            &crate::types::ObjectType {
                id: "game".into(),
                name: "Game".into(),
                schema_json: "{}".into(),
                ui_schema_json: "{}".into(),
                created_at: "2026-01-01T00:00:00.000Z".into(),
                updated_at: "2026-01-01T00:00:00.000Z".into(),
                system_locked: false,
            },
        )
        .unwrap();
        for object_id in ["game-1", "app-v2"] {
            crate::db::upsert_object(
                &conn,
                &crate::types::ArkObject {
                    id: object_id.into(),
                    type_id: "game".into(),
                    type_version: "0.0.0-legacy".into(),
                    title: object_id.into(),
                    content_json: json!({}),
                    props_json: json!({}),
                    created_at: "2026-01-01T00:00:00.000Z".into(),
                    updated_at: "2026-01-01T00:00:00.000Z".into(),
                    deleted_at: None,
                },
            )
            .unwrap();
        }
        for (id, path) in [
            ("app-v1", "c:\\games\\demo\\app-1.0.1\\game.exe"),
            ("app-v2", "c:\\games\\demo\\app-1.0.2\\game.exe"),
        ] {
            crate::db::upsert_tracked_app(&conn, &tracked_app(id, path)).unwrap();
        }
        // Pre-update session on the OLD id: 30 min focused, 10 min idle.
        crate::db::upsert_usage_session(&conn, &session("s1", "app-v1", 1_800_000, 600_000))
            .unwrap();
        // Post-update session on the NEW id: 10 min focused.
        crate::db::upsert_usage_session(&conn, &session("s2", "app-v2", 600_000, 0)).unwrap();
        crate::db::upsert_object_link(
            &conn,
            &ObjectLink {
                id: "l1".into(),
                source_object_id: "game-1".into(),
                target_object_id: "app-v2".into(), // link points at the new id
                link_type: "game".into(),
                created_at: "2026-01-01T00:00:00.000Z".into(),
            },
        )
        .unwrap();

        let usage = usage_for(&conn, "game-1").unwrap();
        // 1800s + 600s of ACTIVE time — not the 2×3600s julianday wall clock
        // (which also included idle), and not the 600s the pinned id alone saw.
        assert_eq!(usage.seconds, 2400);
        assert_eq!(usage.session_count, 2);
        assert!(usage.last_played_at.is_some());
    }

    #[test]
    fn wire_contract_is_typed_and_camel_case() {
        let command = GameUpsertCommand {
            id: "g".into(),
            title: "Game".into(),
            play_status: None,
            user_rating: None,
            genres: vec![],
            platforms: vec![],
            released: None,
            description: None,
            extensions: json!({}),
            created_at: "c".into(),
            updated_at: "u".into(),
            deleted_at: None,
            links: vec![],
            local: Default::default(),
            quarantine: Default::default(),
            device_id: None,
        };
        let value = serde_json::to_value(command).unwrap();
        assert!(value.get("playStatus").is_some());
        assert!(value.get("play_status").is_none());
        assert!(value.get("propsJson").is_none());
    }
}
