#[tokio::test]
async fn saving_a_stale_vector_cannot_erase_progress_committed_before_its_write() {
    let backend = Arc::new(MemBackend::new());
    let mut stale = VersionVector::new();
    stale.insert(
        "book-1".into(),
        "2026-10-16T00:00:01.000Z:000001:device-a".into(),
    );
    backend.replace_version_vector(&stale).await.unwrap();

    let mut newer = stale.clone();
    newer.insert(
        "book-1".into(),
        "2026-10-16T00:00:02.000Z:000002:device-a".into(),
    );
    newer.insert(
        "another-entity".into(),
        "2026-10-16T00:00:03.000Z:000003:device-a".into(),
    );
    backend.race_next_vector_merge_with(newer.clone());

    let storage: Arc<dyn SyncRuntimeStorage> = backend.clone();
    save_version_vector(&storage, &stale).await.unwrap();

    assert_eq!(backend.load_version_vector().await.unwrap(), newer);
}
