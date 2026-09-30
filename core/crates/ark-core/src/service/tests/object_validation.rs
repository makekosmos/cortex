use super::*;

#[tokio::test]
async fn legacy_alias_object_write_read_and_filter_is_canonical() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    handle_request(
        &state,
        Request::Init {
            db_path: dir.path().join("ark.db").to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();

    handle_request(
        &state,
        Request::UpsertObject {
            object: ArkObjectWrite {
                id: "alias-note".into(),
                type_id: "com.kosmos.note".into(),
                type_version: Some("1.0.0".into()),
                title: "Alias note".into(),
                content_json: json!({"type":"doc","content":[]}),
                props_json: json!({"description":null,"extensions":{}}),
                created_at: "2026-06-18T00:00:00.000Z".into(),
                updated_at: "2026-06-18T00:00:00.000Z".into(),
                deleted_at: None,
            },
            expected_snapshot: None,
            device_id: None,
        },
    )
    .await
    .unwrap();

    let object = handle_request(
        &state,
        Request::GetObject {
            id: "alias-note".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(object["typeId"], "com.kosmos.note");
    assert_eq!(object["typeVersion"], "1.0.0");
    for type_id in ["note_obj", "com.kosmos.note"] {
        let objects = handle_request(
            &state,
            Request::ListObjectsByType {
                type_id: type_id.into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(objects.as_array().unwrap().len(), 1);
        assert_eq!(objects[0]["id"], "alias-note");
    }
}

/// FK violation → handle_request должен возвращать Err.
/// Текущий код проглатывает ошибку и возвращает Ok(true) → RED.
#[tokio::test]
async fn upsert_object_with_invalid_type_id_is_err() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let db_path = dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    // type_id "nonexistent_type" не существует → FK violation в db::upsert_object
    let object = ArkObjectWrite {
        id: "obj-bad-type".to_string(),
        type_id: "nonexistent_type".to_string(),
        type_version: Some("0.0.0-legacy".to_string()),
        title: "Bad Object".to_string(),
        content_json: json!({}),
        props_json: json!({}),
        created_at: "2026-06-18T00:00:00.000Z".to_string(),
        updated_at: "2026-06-18T00:00:00.000Z".to_string(),
        deleted_at: None,
    };

    let result = handle_request(
        &state,
        Request::UpsertObject {
            object,
            expected_snapshot: None,
            device_id: None,
        },
    )
    .await;

    assert!(
        result.is_err(),
        "UpsertObject с несуществующим type_id должен возвращать Err (FK violation); \
             получено: {:?}",
        result
    );
}

/// После неудачного upsert строки в objects нет И нет записи в sync
/// version-vector для этого id (атомарность, дефект №2).
#[tokio::test]
async fn failed_upsert_object_persists_nothing() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let db_path = dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    // type_id "ghost_type" не существует → upsert упадёт на FK
    let object = ArkObjectWrite {
        id: "obj-ghost".to_string(),
        type_id: "ghost_type".to_string(),
        type_version: Some("0.0.0-legacy".to_string()),
        title: "Ghost".to_string(),
        content_json: json!({}),
        props_json: json!({}),
        created_at: "2026-06-18T00:00:00.000Z".to_string(),
        updated_at: "2026-06-18T00:00:00.000Z".to_string(),
        deleted_at: None,
    };

    // Ожидаем Err; после него проверяем что ничего не записалось
    let _ = handle_request(
        &state,
        Request::UpsertObject {
            object,
            expected_snapshot: None,
            device_id: Some("test-device".to_string()),
        },
    )
    .await;

    let shared = get_shared_conn(&state).unwrap();
    let guard = shared.lock().unwrap();

    // Объект не должен быть в таблице objects
    let obj_count: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM objects WHERE id = ?1",
            rusqlite::params!["obj-ghost"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        obj_count, 0,
        "objects не должны содержать строку для obj-ghost после провального upsert"
    );

    // Version-vector не должен содержать запись для этого id
    let vv_raw = db::get_sync_kv(&guard, "lan_sync.version_vector").unwrap();
    if let Some(raw) = vv_raw {
        let vector: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_str(&raw).unwrap_or_default();
        assert!(
            !vector.contains_key("obj-ghost"),
            "version_vector не должен содержать запись для obj-ghost после провального upsert; \
                 vector: {:?}",
            vector
        );
    }
    // Если vv_raw == None — version_vector ещё не создавался, тест проходит
}
