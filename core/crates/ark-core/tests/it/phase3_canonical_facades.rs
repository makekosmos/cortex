#![allow(clippy::unwrap_used)]
use ark_core::canonical_types::facades::{asset_sources, set_book_cover, AssetSourceError};
use ark_core::db;
use ark_core::types::ArkObject;
use rusqlite::Connection;
use serde_json::json;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_schema(&conn).unwrap();
    conn
}
fn image(id: &str) -> ArkObject {
    ArkObject {
        id: id.into(),
        type_id: "com.kosmos.image".into(),
        type_version: "1.0.0".into(),
        title: "".into(),
        content_json: json!({}),
        props_json: json!(
            {"fileName":null,
            "mimeType":null,
            "sizeBytes":null,
            "width":null,
            "height":null,
            "resolution":null,
            "altText":"",
            "extensions":{}}),
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
        deleted_at: None,
    }
}
fn book(id: &str) -> ArkObject {
    ArkObject {
        id: id.into(),
        type_id: "com.kosmos.book".into(),
        type_version: "1.0.0".into(),
        title: "Book".into(),
        content_json: json!({"type":"doc","content":[]}),
        props_json: json!(
            {"author":null,
            "isbn":null,
            "pageCount":null,
            "language":null,
            "publisher":null,
            "publishedDate":null,
            "sourceUrl":null,
            "extensions":{}}),
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
        deleted_at: None,
    }
}
#[test]
fn asset_sources_prefers_local_and_orders_rich_text_without_legacy_props() {
    let conn = setup();
    db::upsert_object(&conn, &image("img-local")).unwrap();
    conn.execute(
        concat!(
            "INSERT INTO object_local_state(object_id,device_id,data_json,updated_at) ",
            "VALUES('img-local','d','{\"image\":{\"sourcePath\":\"/tmp/local.png\"}}',",
            "'now')"
        ),
        [],
    )
    .unwrap();
    let mut note = book("note-like");
    note.type_id = "com.kosmos.note".into();
    note.props_json = json!({"description":null,"extensions":{}});
    db::upsert_object(&conn, &note).unwrap();
    conn.execute(
        concat!(
            "INSERT INTO object_local_state(object_id,device_id,data_json,updated_at) ",
            "VALUES('note-like','d',",
            "'{\"richTextImages\":{\"kosmos-local://richtext-image/b\":\"/b\",",
            "\"kosmos-local://richtext-image/a\":\"/a\"}}','now')"
        ),
        [],
    )
    .unwrap();
    let rows = asset_sources(&conn, &["note-like".into(), "img-local".into()]).unwrap();
    assert_eq!(rows[0].source_ref, "/tmp/local.png");
    assert_eq!(
        rows[1].token.as_deref(),
        Some("kosmos-local://richtext-image/a")
    );
    assert_eq!(rows.len(), 3);
}
#[test]
fn asset_sources_rejects_bound_and_wrong_inputs() {
    let conn = setup();
    let ids = (0..257).map(|i| format!("{i}")).collect::<Vec<_>>();
    assert_eq!(
        asset_sources(&conn, &ids).unwrap_err().code,
        "too_many_object_ids"
    );
    assert_eq!(asset_sources(&conn, &["missing".into()]).unwrap(), vec![]);
}
#[test]
fn set_book_cover_links_existing_image_and_removes_only_link() {
    let conn = setup();
    db::upsert_object(&conn, &book("book-1")).unwrap();
    db::upsert_object(&conn, &image("image-1")).unwrap();
    let first = set_book_cover(&conn, "book-1", None, Some("image-1"), "", "device").unwrap();
    assert!(first.changed);
    assert_eq!(first.links.len(), 1);
    assert!(first.image.is_some());
    let second = set_book_cover(&conn, "book-1", None, Some("image-1"), "", "device").unwrap();
    assert!(!second.changed);
    let removed = set_book_cover(&conn, "book-1", None, None, "", "device").unwrap();
    assert!(removed.changed);
    assert!(db::list_object_links(&conn).unwrap().is_empty());
    let props: String = conn
        .query_row(
            "SELECT props_json FROM objects WHERE id='book-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!props.contains("cover_image"));
    assert!(db::get_object(&conn, "image-1").unwrap().is_some());
}
#[test]
fn set_book_cover_rejects_source_without_mutation() {
    let conn = setup();
    db::upsert_object(&conn, &book("book-1")).unwrap();
    let before: i64 = conn
        .query_row("SELECT COUNT(*) FROM object_links", [], |r| r.get(0))
        .unwrap();
    let err = set_book_cover(
        &conn,
        "book-1",
        Some("https://example.invalid/cover"),
        None,
        "",
        "d",
    )
    .unwrap_err();
    assert_eq!(err.code, "source_requires_existing_image");
    let after: i64 = conn
        .query_row("SELECT COUNT(*) FROM object_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
}
#[test]
fn set_book_cover_requires_typed_image_target() {
    let conn = setup();
    db::upsert_object(&conn, &book("book-1")).unwrap();
    db::upsert_object(&conn, &book("not-image")).unwrap();
    let err = set_book_cover(&conn, "book-1", None, Some("not-image"), "", "d").unwrap_err();
    assert_eq!(err.code, "wrong_image_type");
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM object_links", [], |r| r.get(0))
            .unwrap(),
        0
    );
    let _ = AssetSourceError::MAX_IDS;
}
