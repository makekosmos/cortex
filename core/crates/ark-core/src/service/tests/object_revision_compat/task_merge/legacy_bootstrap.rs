use super::*;

#[tokio::test]
async fn editing_legacy_task_builds_mutation_from_the_new_bootstrap_baseline() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();

    let id = "legacy-task-bootstrap";
    with_write_tx(&state, |connection| {
        let object =
            crate::canonical_types::ingress::prepare_object(connection, task(id, "before"))
                .map_err(|error| error.to_string())?;
        db::upsert_object(connection, &object)?;
        connection
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted)
                 VALUES (?1, '2026-10-01T00:00:00.000Z:000001:legacy', 0)",
                [id],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    })
    .unwrap();

    let snapshot = object_snapshot(&state, id).await;
    handle_request(
        &state,
        Request::UpsertObject {
            object: task(id, "after"),
            expected_snapshot: Some(snapshot),
            operation_id: None,
            device_id: Some("legacy-mutation-device".into()),
        },
    )
    .await
    .unwrap();

    let (bootstrap_count, title_operations) = with_conn(&state, |connection| {
        let bootstrap_count = connection
            .query_row(
                "SELECT COUNT(*) FROM task_merge_bootstraps WHERE object_id = ?1",
                [id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let title_operations = connection
            .query_row(
                "SELECT COUNT(*) FROM task_merge_operations
                 WHERE object_id = ?1 AND unit = '\"title\"'
                   AND operation_json LIKE '%\"after\"%'",
                [id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        Ok((bootstrap_count, title_operations))
    })
    .unwrap();
    assert_eq!(bootstrap_count, 1);
    assert_eq!(title_operations, 1);
    let object = handle_request(&state, Request::GetObject { id: id.into() })
        .await
        .unwrap();
    assert_eq!(object["title"], "after");
}

#[tokio::test]
async fn deleting_legacy_task_uses_the_same_bootstrap_conflict_set_it_persists() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();

    let id = "legacy-task-delete-bootstrap";
    with_write_tx(&state, |connection| {
        let object =
            crate::canonical_types::ingress::prepare_object(connection, task(id, "before"))
                .map_err(|error| error.to_string())?;
        db::upsert_object(connection, &object)?;
        connection
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted)
                 VALUES (?1, '2026-10-01T00:00:00.000Z:000001:legacy', 0)",
                [id],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    })
    .unwrap();

    let snapshot = object_snapshot(&state, id).await;
    handle_request(
        &state,
        Request::DeleteObject {
            id: id.into(),
            expected_snapshot: Some(snapshot),
            operation_id: None,
            device_id: Some("legacy-delete-device".into()),
        },
    )
    .await
    .unwrap();

    assert!(
        with_conn(&state, |connection| db::get_object(connection, id))
            .unwrap()
            .is_none()
    );
    let bootstrap_count = with_conn(&state, |connection| {
        connection
            .query_row(
                "SELECT COUNT(*) FROM task_merge_bootstraps WHERE object_id = ?1",
                [id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())
    })
    .unwrap();
    assert_eq!(bootstrap_count, 1);
}

#[tokio::test]
async fn failed_local_task_journal_write_rolls_back_the_complete_mutation() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();

    with_write_tx(&state, |connection| {
        let object = crate::canonical_types::ingress::prepare_object(
            connection,
            task("rollback-task-mutation", "before"),
        )
        .map_err(|error| error.to_string())?;
        db::upsert_object(connection, &object)?;
        connection
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted)
                 VALUES ('rollback-task-mutation', '2026-10-01T00:00:00.000Z:000001:legacy', 0)",
                [],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    })
    .unwrap();

    with_write_tx(&state, |connection| {
        connection
            .execute_batch(
                "CREATE TRIGGER reject_local_task_journal
                 BEFORE INSERT ON task_merge_journal
                 WHEN NEW.record_kind = 'operation'
                 BEGIN SELECT RAISE(ABORT, 'injected task journal failure'); END;",
            )
            .map_err(|error| error.to_string())
    })
    .unwrap();

    assert!(handle_request(
        &state,
        Request::UpsertObject {
            object: task("rollback-task-mutation", "failed"),
            expected_snapshot: Some(object_snapshot(&state, "rollback-task-mutation").await),
            operation_id: None,
            device_id: Some("rollback-test-device".into()),
        },
    )
    .await
    .is_err());

    let counts = with_conn(&state, |connection| {
        let object_rows = connection
            .query_row(
                "SELECT COUNT(*) FROM objects WHERE id = 'rollback-task-mutation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let bootstrap_rows = connection
            .query_row(
                "SELECT COUNT(*) FROM task_merge_bootstraps
                 WHERE object_id = 'rollback-task-mutation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let operation_rows = connection
            .query_row(
                "SELECT COUNT(*) FROM task_merge_operations
                 WHERE object_id = 'rollback-task-mutation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let journal_rows = connection
            .query_row(
                "SELECT COUNT(*) FROM task_merge_journal
                 WHERE record_id IN (
                     SELECT bootstrap_id FROM task_merge_bootstraps
                     WHERE object_id = 'rollback-task-mutation'
                 )",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let link_rows = connection
            .query_row(
                "SELECT COUNT(*) FROM object_links
                 WHERE source_object_id = 'rollback-task-mutation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let tombstone_rows = connection
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id = 'rollback-task-mutation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let revision_rows = connection
            .query_row(
                "SELECT COUNT(*) FROM object_sync_versions
                 WHERE object_id = 'rollback-task-mutation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let feed_rows = connection
            .query_row(
                "SELECT COUNT(*) FROM ark_committed_feed
                 WHERE entity_id = 'rollback-task-mutation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let vector: Option<String> =
            ark_store::sync_kv::get_sync_kv(connection, ark_store::VERSION_VECTOR_KEY)?;
        let vector_has_object = vector
            .as_deref()
            .map(serde_json::from_str::<ark_domain::types::VersionVector>)
            .transpose()
            .map_err(|error| error.to_string())?
            .is_some_and(|value| value.contains_key("rollback-task-mutation"));
        Ok((
            object_rows,
            bootstrap_rows,
            operation_rows,
            journal_rows,
            link_rows,
            tombstone_rows,
            revision_rows,
            feed_rows,
            vector_has_object,
        ))
    })
    .unwrap();
    assert_eq!(counts, (1, 0, 0, 0, 0, 0, 1, 0, false));
    let object = handle_request(
        &state,
        Request::GetObject {
            id: "rollback-task-mutation".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(object["title"], "before");
}
