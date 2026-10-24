use std::path::Path;

use ark_domain::types::ArkObject;
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookCoverRollbackState {
    pub book_has_cover: bool,
    pub link_count: i64,
    pub revision_count: i64,
    pub version_vector: Option<String>,
}

/// Seeds the book-cover command fixture and makes revision writes fail.
pub struct BookCoverRollbackFixture;

impl BookCoverRollbackFixture {
    pub fn prepare(path: &Path) -> Result<(), String> {
        let connection = open_database(path)?;
        crate::objects::upsert_object_compat(&connection, &book())
            .map_err(|error| error.to_string())?;
        crate::objects::upsert_object_compat(&connection, &image())
            .map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "CREATE TRIGGER fail_book_sync_version BEFORE INSERT ON object_sync_versions
                 WHEN NEW.object_id = 'book' BEGIN
                   SELECT RAISE(ABORT, 'injected revision failure');
                 END;",
            )
            .map_err(|error| error.to_string())
    }

    pub fn inspect(path: &Path) -> Result<BookCoverRollbackState, String> {
        let connection = open_database(path)?;
        let props_json: String = connection
            .query_row(
                "SELECT props_json FROM objects WHERE id = 'book'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let props: serde_json::Value =
            serde_json::from_str(&props_json).map_err(|error| error.to_string())?;
        let link_count = connection
            .query_row("SELECT COUNT(*) FROM object_links", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        let revision_count = connection
            .query_row(
                "SELECT COUNT(*) FROM object_sync_versions WHERE object_id IN ('book', 'image')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let version_vector = crate::sync_kv::get_sync_kv(&connection, "lan_sync.version_vector")?;
        Ok(BookCoverRollbackState {
            book_has_cover: props
                .as_object()
                .is_some_and(|props| props.contains_key("cover_image")),
            link_count,
            revision_count,
            version_vector,
        })
    }
}

fn open_database(path: &Path) -> Result<rusqlite::Connection, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "book-cover fixture path is not UTF-8".to_string())?;
    crate::open::open_db(path)
}

fn book() -> ArkObject {
    test_object(
        "book",
        "core.book",
        json!({
            "author": null,
            "isbn": null,
            "pageCount": null,
            "language": null,
            "publisher": null,
            "publishedDate": null,
            "sourceUrl": null,
            "extensions": {},
        }),
    )
}

fn image() -> ArkObject {
    test_object(
        "image",
        "core.image",
        json!({
            "fileName": null,
            "mimeType": null,
            "sizeBytes": null,
            "width": null,
            "height": null,
            "resolution": null,
            "altText": "",
            "extensions": {},
        }),
    )
}

fn test_object(id: &str, type_id: &str, props_json: serde_json::Value) -> ArkObject {
    ArkObject {
        id: id.into(),
        type_id: type_id.into(),
        type_version: "0.1.0".into(),
        title: "test".into(),
        content_json: json!({"type":"doc","content":[]}),
        props_json,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
        deleted_at: None,
    }
}
