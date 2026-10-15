use super::*;
use crate::types::ArkObject;

fn object(id: &str, type_id: &str, props: Value) -> ArkObject {
    ArkObject {
        id: id.into(),
        type_id: type_id.into(),
        type_version: "1.0.0".into(),
        title: "test".into(),
        content_json: json!({"type":"doc","content":[]}),
        props_json: props,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
        deleted_at: None,
    }
}

#[tokio::test]
async fn book_cover_command_rolls_back_facade_writes_when_revision_recording_fails() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    with_write_tx(&state, |connection| {
        db::upsert_object(
            connection,
            &object(
                "book",
                "com.kosmos.book",
                json!({"author":null,"isbn":null,"pageCount":null,"language":null,"publisher":null,"publishedDate":null,"sourceUrl":null,"extensions":{}}),
            ),
        )?;
        db::upsert_object(
            connection,
            &object(
                "image",
                "com.kosmos.image",
                json!({"fileName":null,"mimeType":null,"sizeBytes":null,"width":null,"height":null,"resolution":null,"altText":"","extensions":{}}),
            ),
        )?;
        connection
            .execute_batch(
                "CREATE TRIGGER fail_book_sync_version BEFORE INSERT ON object_sync_versions
                 WHEN NEW.object_id = 'book' BEGIN SELECT RAISE(ABORT, 'injected revision failure'); END;",
            )
            .map_err(|error| error.to_string())
    })
    .unwrap();

    assert!(handle_request(
        &state,
        Request::CanonicalSetBookCover {
            book_id: "book".into(),
            source_ref: None,
            existing_image_id: Some("image".into()),
            alt_text: String::new(),
            device_id: Some("command-test".into()),
        },
    )
    .await
    .is_err());

    with_conn(&state, |connection| {
        let props: String = connection
            .query_row(
                "SELECT props_json FROM objects WHERE id='book'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let links: i64 = connection
            .query_row("SELECT COUNT(*) FROM object_links", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        let revisions: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM object_sync_versions WHERE object_id IN ('book', 'image')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        assert!(
            !props.contains("cover_image"),
            "failed command must restore book props"
        );
        assert_eq!(links, 0, "failed command must remove facade-created links");
        assert_eq!(
            revisions, 0,
            "failed command must roll back object versions"
        );
        assert_eq!(
            db::get_sync_kv(connection, "lan_sync.version_vector")?,
            None
        );
        Ok(())
    })
    .unwrap();
}
