// Arrancador backend — scanner + launcher + RAWG + SQOBA модули.
//
// Subagent A scope: `scanner.rs`, `launcher.rs`, `config.rs`. RAWG (`rawg.rs`)
// и SQOBA (`sqoba.rs`) — subagent B / C соответственно. WS-диспатчер
// `handle_arrancador_op` живёт в `ws_server.rs` и регистрирует sub-operations
// `arrancador.scan` + `arrancador.launch` (RAWG / SQOBA — будут зарегистрированы
// другими subagent'ами и сливаются в один match).

pub mod config;
pub mod game_facade;
pub mod launcher;
pub mod rawg;
pub mod scanner;
pub mod sqoba;

use serde_json::{json, Value};

pub(crate) const GAME_TYPE: &str = "com.kosmos.game";
pub(crate) const GAME_VERSION: &str = "1.0.0";

pub(crate) fn project_game(
    record: &Value,
    links: &Value,
    targets: &Value,
) -> Result<Value, &'static str> {
    let object_type = record
        .get("typeId")
        .and_then(Value::as_str)
        .ok_or("malformed_game")?;
    if object_type != GAME_TYPE {
        return Err("wrong_type");
    }
    let version = record
        .get("typeVersion")
        .and_then(Value::as_str)
        .ok_or("malformed_game")?;
    if version != GAME_VERSION {
        return Err("wrong_version");
    }
    let id = record
        .get("id")
        .and_then(Value::as_str)
        .ok_or("malformed_game")?;
    let props = record
        .get("propsJson")
        .and_then(Value::as_object)
        .ok_or("malformed_game")?;
    let get = |canonical: &str| props.get(canonical);

    let platforms = get("platforms")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut cover = Value::Null;
    let target_values = targets.as_array().ok_or("malformed_link_target")?;
    for link in links.as_array().ok_or("malformed_links")? {
        let source = link
            .get("sourceObjectId")
            .or_else(|| link.get("source_object_id"))
            .and_then(Value::as_str);
        let target_id = link
            .get("targetObjectId")
            .or_else(|| link.get("target_object_id"))
            .and_then(Value::as_str);
        let link_type = link
            .get("linkType")
            .or_else(|| link.get("link_type"))
            .and_then(Value::as_str);
        if source != Some(id) || target_id.is_none() {
            continue;
        }
        let target = target_values
            .iter()
            .find(|t| t.get("id").and_then(Value::as_str) == target_id);
        if link_type == Some("cover") {
            let target = target.ok_or("malformed_link_target")?;
            let target_type = target
                .get("typeId")
                .and_then(Value::as_str)
                .or_else(|| target.get("type_id").and_then(Value::as_str))
                .ok_or("malformed_link_target")?;
            let target_version = target
                .get("typeVersion")
                .and_then(Value::as_str)
                .or_else(|| target.get("type_version").and_then(Value::as_str))
                .unwrap_or("");
            if target_type != "com.kosmos.image" || target_version != GAME_VERSION {
                return Err("wrong_link_type");
            }
            let target_props = target
                .get("propsJson")
                .or_else(|| target.get("props_json"))
                .and_then(Value::as_object)
                .ok_or("malformed_link_target")?;
            cover = json!({"imageId": target_id, "sourceRef": target_props.get("sourceRef").or_else(|| target_props.get("source_ref")).and_then(Value::as_str)});
        }
    }
    let mut seconds = 0i64;
    let mut count = 0u64;
    for link in links.as_array().ok_or("malformed_links")? {
        if link
            .get("sourceObjectId")
            .or_else(|| link.get("source_object_id"))
            .and_then(Value::as_str)
            != Some(id)
            || link
                .get("linkType")
                .or_else(|| link.get("link_type"))
                .and_then(Value::as_str)
                != Some("session")
        {
            continue;
        }
        let target_id = link
            .get("targetObjectId")
            .or_else(|| link.get("target_object_id"))
            .and_then(Value::as_str);
        let target = target_values
            .iter()
            .find(|t| t.get("id").and_then(Value::as_str) == target_id)
            .ok_or("malformed_link_target")?;
        if target
            .get("typeId")
            .and_then(Value::as_str)
            .or_else(|| target.get("type_id").and_then(Value::as_str))
            .unwrap_or("")
            != "com.kosmos.time-entry"
        {
            return Err("wrong_link_type");
        }
        let p = target
            .get("propsJson")
            .or_else(|| target.get("props_json"))
            .and_then(Value::as_object)
            .ok_or("malformed_link_target")?;
        if let (Some(start), Some(end)) = (
            p.get("startedAt")
                .or_else(|| p.get("started_at"))
                .and_then(Value::as_str),
            p.get("endedAt")
                .or_else(|| p.get("ended_at"))
                .and_then(Value::as_str),
        ) {
            let start =
                chrono::DateTime::parse_from_rfc3339(start).map_err(|_| "malformed_session")?;
            let end = chrono::DateTime::parse_from_rfc3339(end).map_err(|_| "malformed_session")?;
            seconds += (end - start).num_seconds().max(0);
            count += 1;
        }
    }
    Ok(json!({
        "id": id, "typeId": GAME_TYPE, "typeVersion": GAME_VERSION,
        "title": record.get("title").and_then(Value::as_str).unwrap_or(""),
        "platforms": platforms,
        "playStatus": get("playStatus").cloned().unwrap_or(Value::Null),
        "rating": get("userRating").cloned().unwrap_or(Value::Null),
        "dates": {"createdAt": record.get("createdAt").or_else(|| record.get("created_at")).cloned().unwrap_or(Value::Null), "updatedAt": record.get("updatedAt").or_else(|| record.get("updated_at")).cloned().unwrap_or(Value::Null), "released": get("released").cloned().unwrap_or(Value::Null)},
        "cover": cover, "sessionTotals": {"seconds": seconds, "count": count}
    }))
}

#[cfg(test)]
mod canonical_game_tests {
    use super::project_game;
    use serde_json::json;

    #[test]
    fn canonical_game_projection_excludes_legacy_props_and_derives_cover_link() {
        let record = json!({
            "id": "game-1",
            "typeId": "com.kosmos.game",
            "typeVersion": "1.0.0",
            "title": "Canonical Game",
            "propsJson": {
                "playStatus": "inProgress",
                "userRating": 9,
                "released": "2026-01-02",
                "platforms": ["PC"],
                "raw_props": "must-not-leak"
            },
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-03T00:00:00Z",
            "deletedAt": null
        });
        let links = json!([{
            "sourceObjectId": "game-1",
            "targetObjectId": "image-1",
            "linkType": "cover"
        }, {
            "sourceObjectId": "game-1",
            "targetObjectId": "session-1",
            "linkType": "session"
        }]);
        let targets = json!([{
            "id": "image-1", "typeId": "com.kosmos.image", "typeVersion": "1.0.0",
            "propsJson": {"sourceRef": "asset://cover"}
        }, {
            "id": "session-1", "typeId": "com.kosmos.time-entry", "typeVersion": "1.0.0",
            "propsJson": {"startedAt": "2026-01-01T00:00:00Z", "endedAt": "2026-01-01T01:30:00Z"}
        }]);
        let projected = project_game(&record, &links, &targets).expect("projection");
        assert_eq!(projected["id"], "game-1");
        assert_eq!(projected["typeId"], "com.kosmos.game");
        assert_eq!(projected["cover"]["sourceRef"], "asset://cover");
        assert_eq!(projected["sessionTotals"]["seconds"], 5400);
        assert!(projected.get("raw_props").is_none());
    }

    #[test]
    fn legacy_alias_is_rejected_by_runtime_projection() {
        let legacy = json!({"id":"legacy","typeId":"game_obj","typeVersion":"1.0.0","title":"Old","propsJson":{},"createdAt":"x","updatedAt":"x","deletedAt":null});
        assert!(project_game(&legacy, &json!([]), &json!([])).is_err());
        let wrong = json!({"id":"bad","typeId":"com.kosmos.note","typeVersion":"1.0.0","title":"Bad","propsJson":{},"createdAt":"x","updatedAt":"x","deletedAt":null});
        assert_eq!(
            project_game(&wrong, &json!([]), &json!([])).unwrap_err(),
            "wrong_type"
        );
    }
}
