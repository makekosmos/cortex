use super::*;

#[test]
fn operation_mode_task_snapshots_are_skipped_with_cursor_advance() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(ark_store::schema::CREATE_TABLES)
        .unwrap();
    ark_store::data_platform::ensure_schema(&conn).unwrap();
    append_committed_change(&conn, "object", "merged-task", "r1", false, None, None).unwrap();
    conn.execute(
        "INSERT INTO task_merge_bootstraps(
             bootstrap_id, object_id, bootstrap_json, payload_sha256
         ) VALUES ('00000000000000000000000000000001', 'merged-task', '{}', 'digest')",
        [],
    )
    .unwrap();

    assert_eq!(
        read_committed_feed_delivery_page(&conn, 0, 1, 8).unwrap(),
        CommittedFeedDeliveryPage::SkipTo {
            through_position: 1
        }
    );
}

#[test]
fn narrowing_preserves_local_copy_and_widening_replays_latest_state() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("policy-source.db");
    let receiver_path = directory.path().join("policy-receiver.db");
    let source = crate::db::open_db(source_path.to_str().unwrap()).unwrap();
    crate::db::init_schema(&source).unwrap();
    let receiver = crate::db::open_db(receiver_path.to_str().unwrap()).unwrap();
    crate::db::init_schema(&receiver).unwrap();
    let book_type = crate::types::ObjectType {
        id: "book".into(),
        name: "Book".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "2026-10-16T00:00:00.000Z".into(),
        updated_at: "2026-10-16T00:00:00.000Z".into(),
        system_locked: false,
    };
    crate::db::upsert_object_type(&source, &book_type).unwrap();
    crate::db::upsert_object_type(&receiver, &book_type).unwrap();

    let original = crate::types::ArkObject {
        id: "book-1".into(),
        type_id: "book".into(),
        type_version: "0.0.0-legacy".into(),
        title: "Original title".into(),
        content_json: serde_json::json!({"text":"original"}),
        props_json: serde_json::json!({}),
        created_at: "2026-10-16T00:00:00.000Z".into(),
        updated_at: "2026-10-16T00:00:00.000Z".into(),
        deleted_at: None,
    };
    crate::db::upsert_object(&source, &original).unwrap();
    crate::db::upsert_object(&receiver, &original).unwrap();
    append_committed_change(&source, "object", &original.id, "r1", false, None, None).unwrap();

    let rule = |mode| ark_application::sync_policy::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "book".into(),
        mode,
        filter: ark_domain::selective_sync::FilterV1::match_all(),
    };
    let excluded_rule = rule(ark_domain::selective_sync::SyncMode::None);
    let excluded_digest =
        ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&excluded_rule));
    let excluded = ark_store::data_platform::accept_peer_sync_policy(
        &source,
        "receiver-device",
        1,
        &excluded_digest,
        true,
        std::slice::from_ref(&excluded_rule),
    )
    .unwrap()
    .identity();
    let initial = begin_outbound_policy_identity(&source, "receiver-device", &excluded).unwrap();
    complete_outbound_snapshot_identity(&source, "receiver-device", &excluded).unwrap();
    assert_eq!(initial.initial_snapshot.through(), 1);

    let mut changed = original.clone();
    changed.title = "Updated while excluded".into();
    changed.content_json = serde_json::json!({"text":"new version"});
    changed.updated_at = "2026-10-16T00:00:01.000Z".into();
    crate::db::upsert_object(&source, &changed).unwrap();
    append_committed_change(&source, "object", &changed.id, "r2", false, None, None).unwrap();
    let narrowed =
        read_committed_feed_delivery_page_for_peer(&source, "receiver-device", &excluded, 1, 2, 8)
            .unwrap();
    assert_eq!(
        narrowed.page,
        CommittedFeedDeliveryPage::SkipTo {
            through_position: 2
        }
    );
    record_outbound_scan_identity(&source, "receiver-device", &excluded, 2).unwrap();

    let retained = crate::db::get_object(&receiver, "book-1")
        .unwrap()
        .expect("narrowing never deletes an existing local row");
    assert_eq!(retained.title, "Original title");
    assert!(retained.deleted_at.is_none());

    let included_rule = rule(ark_domain::selective_sync::SyncMode::Full);
    let included_digest =
        ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&included_rule));
    let included = ark_store::data_platform::accept_peer_sync_policy(
        &source,
        "receiver-device",
        2,
        &included_digest,
        true,
        std::slice::from_ref(&included_rule),
    )
    .unwrap()
    .identity();
    let reset = begin_outbound_policy_identity(&source, "receiver-device", &included).unwrap();
    assert_eq!(
        reset.applied_through, 0,
        "widening restarts delivery progress"
    );
    assert_eq!(
        reset.scanned_through, 0,
        "widening rescans prior feed positions"
    );

    let replay =
        read_committed_feed_delivery_page_for_peer(&source, "receiver-device", &included, 0, 2, 8)
            .unwrap();
    let CommittedFeedDeliveryPage::Deliveries(rows) = replay.page else {
        panic!("current state should fit into one feed page");
    };
    assert_eq!(
        rows.len(),
        1,
        "feed coalesces old versions to current state"
    );
    assert_eq!(rows[0].entry.position, 2);
    assert_eq!(
        rows[0].entity.data.get("title"),
        Some(&serde_json::json!("Updated while excluded"))
    );
    let retained = crate::db::get_object(&receiver, "book-1")
        .unwrap()
        .expect("replay planning does not remove the existing local row");
    assert_eq!(retained.title, "Original title");
}
