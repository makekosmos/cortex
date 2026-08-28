use super::*;

fn task(id: &str, title: &str) -> ArkObjectWrite {
    let mut object = canonical_task_object(Some("1.0.0"), canonical_task_props());
    object.id = id.to_string();
    object.title = title.to_string();
    object
}

fn remote_entity(object: &ArkObject, title: &str) -> SyncEntity {
    let Value::Object(mut data) = serde_json::to_value(object).unwrap() else {
        unreachable!()
    };
    data.remove("id");
    data.insert("title".to_string(), json!(title));
    SyncEntity {
        entity_type: "object".to_string(),
        id: object.id.clone(),
        data,
        hlc: "2026-01-01T00:00:00.000Z:000001:remote".to_string(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    }
}

#[tokio::test]
async fn pre_version_table_revisions_reject_stale_remote_resurrection() {
    let _guard = TEST_DB_MUTEX.lock().await;
    let dir = tempfile::tempdir().unwrap();
    handle_request(Request::Init {
        db_path: dir.path().join("ark.db").to_string_lossy().to_string(),
    })
    .await
    .unwrap();

    let (vector_object, tombstone_object) = with_write_tx(|conn| {
        let vector_object = ark_core::canonical_types::ingress::prepare_object(
            conn,
            task("legacy-vector-task", "vector wins"),
        )
        .map_err(|error| error.to_string())?;
        db::upsert_object(conn, &vector_object)?;
        db::set_sync_kv(
            conn,
            "lan_sync.version_vector",
            r#"{"legacy-vector-task":"2999-01-01T00:00:00.000Z:000001:legacy"}"#,
        )?;

        let tombstone_object = ark_core::canonical_types::ingress::prepare_object(
            conn,
            task("legacy-tombstone-task", "deleted"),
        )
        .map_err(|error| error.to_string())?;
        db::upsert_object(conn, &tombstone_object)?;
        db::delete_object(conn, &tombstone_object.id)?;
        db::record_sync_tombstone(
            conn,
            "object",
            &tombstone_object.id,
            "2999-01-01T00:00:00.000Z:000002:legacy",
        )?;
        Ok((vector_object, tombstone_object))
    })
    .unwrap();

    let backend = SqliteStorageBackend::new(get_shared_conn().unwrap());
    backend
        .apply_entity(&remote_entity(&vector_object, "stale vector overwrite"))
        .await
        .unwrap();
    backend
        .apply_entity(&remote_entity(&tombstone_object, "stale resurrection"))
        .await
        .unwrap();

    assert_eq!(
        with_conn(|conn| db::get_object(conn, &vector_object.id))
            .unwrap()
            .unwrap()
            .title,
        "vector wins"
    );
    assert!(with_conn(|conn| db::get_object(conn, &tombstone_object.id))
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn max_remote_hlc_counter_rolls_back_local_write() {
    let _guard = TEST_DB_MUTEX.lock().await;
    let dir = tempfile::tempdir().unwrap();
    handle_request(Request::Init {
        db_path: dir.path().join("ark.db").to_string_lossy().to_string(),
    })
    .await
    .unwrap();
    handle_request(Request::UpsertObject {
        object: task("max-counter-task", "original"),
        expected_snapshot: None,
        device_id: Some("local-writer".to_string()),
    })
    .await
    .unwrap();
    with_write_tx(|conn| {
        conn.execute(
            "UPDATE object_sync_versions SET hlc = ?2 WHERE object_id = ?1",
            rusqlite::params![
                "max-counter-task",
                "2999-01-01T00:00:00.000Z:18446744073709551615:remote"
            ],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
    .unwrap();
    let snapshot: ObjectWriteSnapshot = serde_json::from_value(
        handle_request(Request::GetObjectWriteSnapshot {
            id: "max-counter-task".to_string(),
        })
        .await
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        handle_request(Request::UpsertObject {
            object: task("max-counter-task", "must roll back"),
            expected_snapshot: Some(snapshot),
            device_id: Some("local-writer".to_string()),
        })
        .await
        .unwrap_err(),
        "hlc_counter_overflow"
    );
    assert_eq!(
        with_conn(|conn| db::get_object(conn, "max-counter-task"))
            .unwrap()
            .unwrap()
            .title,
        "original"
    );
}
