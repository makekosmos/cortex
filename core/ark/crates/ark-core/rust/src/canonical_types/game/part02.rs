
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
    let row: Option<(i64, i64, Option<String>)> = conn.query_row(
        "SELECT COALESCE(SUM(CAST((julianday(COALESCE(s.ended_at,s.started_at))-julianday(s.started_at))*86400 AS INTEGER)),0), COUNT(s.id), MAX(COALESCE(s.ended_at,s.started_at)) FROM usage_sessions s JOIN object_links l ON l.target_object_id=s.tracked_app_id WHERE l.source_object_id=?1 AND l.link_type IN ('game','game-usage')",
        params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().map_err(|e| e.to_string())?;
    let (seconds, count, last) = row.unwrap_or((0, 0, None));
    Ok(GameUsage {
        seconds: seconds.max(0),
        session_count: count.max(0) as u64,
        last_played_at: last,
    })
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
