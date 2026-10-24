#![allow(clippy::unwrap_used)]
use crate::canonical_asset_sources::asset_sources;
use ark_application::book_cover::LocalBookCoverWrite;
use ark_domain::types::ArkObject;
use rusqlite::Connection;
use serde_json::json;

fn setup() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&connection).unwrap();
    connection
}
fn image(id: &str) -> ArkObject {
    ArkObject {
        id: id.into(),
        type_id: "core.image".into(),
        type_version: "0.1.0".into(),
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
        type_id: "core.book".into(),
        type_version: "0.1.0".into(),
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
fn set_book_cover(
    conn: &mut Connection,
    book_id: &str,
    source_ref: Option<&str>,
    existing_image_id: Option<&str>,
) -> Result<bool, crate::writer::TransactionFailure<crate::book_cover::BookCoverOperationError>> {
    crate::book_cover::apply_local_book_cover(
        conn,
        LocalBookCoverWrite {
            book_id: book_id.into(),
            source_ref: source_ref.map(str::to_owned),
            existing_image_id: existing_image_id.map(str::to_owned),
            alt_text: String::new(),
            device_id: "device".into(),
        },
    )
}
#[test]
fn asset_sources_prefers_local_and_orders_rich_text_without_legacy_props() {
    let conn = setup();
    crate::objects::upsert_object_compat(&conn, &image("img-local")).unwrap();
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
    note.type_id = "core.note".into();
    note.props_json = json!({"description":null,"extensions":{}});
    crate::objects::upsert_object_compat(&conn, &note).unwrap();
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
    let mut conn = setup();
    crate::objects::upsert_object_compat(&conn, &book("book-1")).unwrap();
    crate::objects::upsert_object_compat(&conn, &image("image-1")).unwrap();
    crate::sync_tombstones::record(
        &conn,
        "object",
        "book-1",
        Some("core.book"),
        "2020-01-01T00:00:00.000Z:000000:device",
    )
    .unwrap();
    crate::sync_tombstones::record(
        &conn,
        "object",
        "image-1",
        Some("core.image"),
        "2020-01-01T00:00:00.000Z:000000:device",
    )
    .unwrap();
    assert!(set_book_cover(&mut conn, "book-1", None, Some("image-1")).unwrap());
    let link_id = crate::objects::list_object_links(&conn).unwrap()[0]
        .id
        .clone();
    assert_eq!(crate::objects::list_object_links(&conn).unwrap().len(), 1);
    assert_eq!(
        crate::sync_tombstones::load_all(&conn)
            .unwrap()
            .iter()
            .filter(|entity| entity.id == "book-1" || entity.id == "image-1")
            .count(),
        0
    );
    assert!(!set_book_cover(&mut conn, "book-1", None, Some("image-1")).unwrap());
    assert!(set_book_cover(&mut conn, "book-1", None, None).unwrap());
    assert!(crate::sync_tombstones::load_all(&conn)
        .unwrap()
        .iter()
        .any(|entity| entity.id == link_id));
    assert!(crate::objects::list_object_links(&conn).unwrap().is_empty());
    let props: String = conn
        .query_row(
            "SELECT props_json FROM objects WHERE id='book-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!props.contains("cover_image"));
    assert!(crate::objects::get_object(&conn, "image-1")
        .unwrap()
        .is_some());
}
#[test]
fn set_book_cover_rejects_source_without_mutation() {
    let mut conn = setup();
    crate::objects::upsert_object_compat(&conn, &book("book-1")).unwrap();
    let before: i64 = conn
        .query_row("SELECT COUNT(*) FROM object_links", [], |r| r.get(0))
        .unwrap();
    let err = set_book_cover(
        &mut conn,
        "book-1",
        Some("https://example.invalid/cover"),
        None,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        crate::writer::TransactionFailure::Operation(
            crate::book_cover::BookCoverOperationError::Invalid(ref error)
        ) if error.code == "source_requires_existing_image"
    ));
    let after: i64 = conn
        .query_row("SELECT COUNT(*) FROM object_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
}
#[test]
fn set_book_cover_requires_typed_image_target() {
    let mut conn = setup();
    crate::objects::upsert_object_compat(&conn, &book("book-1")).unwrap();
    crate::objects::upsert_object_compat(&conn, &book("not-image")).unwrap();
    let err = set_book_cover(&mut conn, "book-1", None, Some("not-image")).unwrap_err();
    assert!(matches!(
        err,
        crate::writer::TransactionFailure::Operation(
            crate::book_cover::BookCoverOperationError::Invalid(ref error)
        ) if error.code == "wrong_image_type"
    ));
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM object_links", [], |r| r.get(0))
            .unwrap(),
        0
    );
}
