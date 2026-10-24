use super::*;

#[tokio::test]
async fn canonical_object_write_read_and_filter_uses_core_type() {
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
                type_id: "core.note".into(),
                type_version: Some("0.1.0".into()),
                title: "Alias note".into(),
                content_json: json!({"type":"doc","content":[]}),
                props_json: json!({"description":null,"extensions":{}}),
                created_at: "2026-06-18T00:00:00.000Z".into(),
                updated_at: "2026-06-18T00:00:00.000Z".into(),
                deleted_at: None,
            },
            expected_snapshot: None,
            operation_id: None,
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
    assert_eq!(object["typeId"], "core.note");
    assert_eq!(object["typeVersion"], "0.1.0");
    for type_id in ["core.note"] {
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
            operation_id: None,
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
            operation_id: None,
            device_id: Some("test-device".to_string()),
        },
    )
    .await;

    let persisted =
        ark_store::test_support::ObjectValidationFixture::inspect(&db_path, "obj-ghost").unwrap();
    assert_eq!(
        persisted,
        ark_store::test_support::ObjectValidationState {
            object_exists: false,
            version_vector_contains_object: false,
        }
    );
}
