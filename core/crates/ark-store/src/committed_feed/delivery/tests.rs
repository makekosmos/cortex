use super::{
    read_committed_feed_delivery_page_for_peer, read_committed_feed_delivery_page_with_hook,
};
use crate::committed_feed::append_committed_change;
use ark_application::sync_storage::{CommittedFeedDeliveryPage, PolicyBoundFeedReadError};
use ark_domain::sync_policy::PolicyIdentity;
use ark_domain::types::ArkObject;

#[test]
fn projection_keeps_selected_locator_and_payload_in_one_read_snapshot() {
    let directory = crate::test_support::test_temp_dir().expect("temporary directory opens");
    let path = directory.path().join("feed-snapshot.db");
    let path = path.to_str().expect("temporary path is UTF-8");
    let reader = crate::open::open_db(path).expect("reader opens");
    reader
        .execute_batch(crate::schema::CREATE_TABLES)
        .expect("schema initializes");
    reader
        .execute(
            "INSERT INTO object_types(
                 id, name, schema_json, ui_schema_json, created_at, updated_at
             )
             VALUES ('core.note', 'Note', '{}', '{}', '2026-10-16T00:00:00.000Z',
                     '2026-10-16T00:00:00.000Z')",
            [],
        )
        .expect("object type persists");
    let writer = crate::open::open_db(path).expect("writer opens");
    writer
        .execute_batch(crate::schema::CREATE_TABLES)
        .expect("writer schema initializes");
    let initial = ArkObject {
        id: "snapshot-object".into(),
        type_id: "core.note".into(),
        type_version: "0.1.0".into(),
        title: "Selected payload".into(),
        content_json: serde_json::json!({"type":"doc","content":[]}),
        props_json: serde_json::json!({}),
        created_at: "2026-10-16T00:00:00.000Z".into(),
        updated_at: "2026-10-16T00:00:00.000Z".into(),
        deleted_at: None,
    };
    crate::objects::upsert_object(&reader, &initial).expect("initial object persists");
    append_committed_change(&reader, "object", &initial.id, "r1", false, None, None)
        .expect("initial feed entry persists");

    let page = read_committed_feed_delivery_page_with_hook(&reader, 0, 1, 8, 1_000_000, || {
        let mut updated = initial.clone();
        updated.title = "Newer concurrent payload".into();
        updated.updated_at = "2026-10-16T00:00:01.000Z".into();
        crate::objects::upsert_object(&writer, &updated)
            .expect("concurrent object update persists");
        append_committed_change(&writer, "object", &updated.id, "r2", false, None, None)
            .expect("concurrent feed entry persists");
    })
    .expect("delivery page reads");

    let deliveries = match page {
        CommittedFeedDeliveryPage::Deliveries(deliveries) => deliveries,
        CommittedFeedDeliveryPage::SkipTo { .. } => panic!("feed page only skipped entries"),
        CommittedFeedDeliveryPage::TooLarge { .. } => panic!("small test payload is bounded"),
    };
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].entry.position, 1);
    assert_eq!(deliveries[0].entry.revision, "r1");
    assert_eq!(
        deliveries[0].entity.data.get("title"),
        Some(&serde_json::json!("Selected payload"))
    );
}

#[test]
fn policy_bound_feed_read_preserves_stale_policy_as_a_typed_outcome() {
    let connection = rusqlite::Connection::open_in_memory().expect("database opens");
    connection
        .execute_batch(crate::schema::CREATE_TABLES)
        .expect("schema initializes");
    crate::data_platform::ensure_schema(&connection).expect("policy schema initializes");
    let stale_identity = PolicyIdentity {
        revision: 1,
        digest: "stale-policy-digest".into(),
        active: true,
    };

    let result = read_committed_feed_delivery_page_for_peer(
        &connection,
        "phone",
        &stale_identity,
        0,
        0,
        8,
        1_000_000,
    );

    assert_eq!(result, Err(PolicyBoundFeedReadError::StalePolicy));
}
