use super::*;

#[tokio::test]
async fn book_cover_command_rolls_back_facade_writes_when_revision_recording_fails() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().to_string())
        .await
        .unwrap();

    ark_store::test_support::BookCoverRollbackFixture::prepare(&fixture.db_path())
        .expect("book and image fixtures seed, then revision writes are rejected");

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

    let state = ark_store::test_support::BookCoverRollbackFixture::inspect(&fixture.db_path())
        .expect("persisted rollback state reads");
    assert!(
        !state.book_has_cover,
        "failed command must restore book props"
    );
    assert_eq!(
        state.link_count, 0,
        "failed command must remove created links"
    );
    assert_eq!(
        state.revision_count, 0,
        "failed command must roll back versions"
    );
    assert_eq!(
        state.version_vector, None,
        "failed command must not advance sync state"
    );
}
