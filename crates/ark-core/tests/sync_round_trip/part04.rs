
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_state_is_not_exported_or_applied_to_a_remote_database() {
    let connection_a = Connection::open_in_memory().unwrap();
    ark_core::db::init_schema(&connection_a).unwrap();
    let shared_a = Arc::new(StdMutex::new(connection_a));
    let backend_a = Arc::new(SqliteStorageBackend::new(shared_a.clone()));
    backend_a.set_device_id("device-a").unwrap();

    let object_type = ark_core::types::ObjectType {
        id: "com.kosmos.note".into(),
        name: "Note".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "2026-09-06T00:00:00Z".into(),
        updated_at: "2026-09-06T00:00:00Z".into(),
        system_locked: false,
    };
    let object = ark_core::types::ArkObject {
        id: "local-only-note".into(),
        type_id: object_type.id.clone(),
        type_version: "1.0.0".into(),
        title: "Export boundary".into(),
        content_json: serde_json::json!({"type":"doc","content":[]}),
        props_json: serde_json::json!({"description":null,"extensions":{}}),
        created_at: "2026-09-06T00:00:00Z".into(),
        updated_at: "2026-09-06T00:00:00Z".into(),
        deleted_at: None,
    };
    {
        let conn = shared_a.lock().unwrap();
        ark_core::db::upsert_object_type(&conn, &object_type).unwrap();
        ark_core::db::upsert_object(&conn, &object).unwrap();
        conn.execute(
            "INSERT INTO object_local_state(object_id,device_id,data_json,updated_at) VALUES(?1,?2,?3,?4)",
            rusqlite::params![
                object.id,
                "device-a",
                r#"{"sourcePath":"C:\\private\\note.md","windowState":{"x":7},"processId":42}"#,
                "2026-09-06T00:00:00Z"
            ],
        )
        .unwrap();
    }

    let exported = backend_a.load_entities(&HashMap::new()).await;
    let exported_object = exported
        .into_iter()
        .find(|entity| entity.entity_type == "object" && entity.id == "local-only-note")
        .expect("canonical object should be exported");
    let exported_json = serde_json::to_string(&exported_object.data).unwrap();
    assert!(!exported_json.contains("sourcePath"));
    assert!(!exported_json.contains("windowState"));
    assert!(!exported_json.contains("processId"));

    let connection_b = Connection::open_in_memory().unwrap();
    ark_core::db::init_schema(&connection_b).unwrap();
    let shared_b = Arc::new(StdMutex::new(connection_b));
    let backend_b = Arc::new(SqliteStorageBackend::new(shared_b.clone()));
    backend_b.set_device_id("device-b").unwrap();
    {
        let conn = shared_b.lock().unwrap();
        ark_core::db::upsert_object_type(&conn, &object_type).unwrap();
    }
    backend_b.apply_entity(&exported_object).await.unwrap();

    let conn = shared_b.lock().unwrap();
    let local_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM object_local_state WHERE object_id=?1",
            rusqlite::params!["local-only-note"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(local_rows, 0, "local-only state must not cross the sync boundary");
    let stored_props: String = conn
        .query_row(
            "SELECT props_json FROM objects WHERE id=?1",
            rusqlite::params!["local-only-note"],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!stored_props.contains("sourcePath"));
    assert!(!stored_props.contains("windowState"));
    assert!(!stored_props.contains("processId"));
}
