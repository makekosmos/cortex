use super::*;

#[tokio::test]
async fn journal_pages_pin_watermarks_and_match_authenticated_peer() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    append(&handle, operation('1', &[], "first"), None)
        .await
        .unwrap();
    append(&handle, operation('2', &['1'], "second"), None)
        .await
        .unwrap();

    let page = handle
        .query(
            CommandContext::authenticated_peer("peer-a".into()),
            TaskMergeJournalPageQuery::new("peer-a".into(), 0, 1, 1),
        )
        .await
        .unwrap();
    assert_eq!(page.through_position, 1);
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].operation.id, id('1'));

    let next = handle
        .query(
            CommandContext::authenticated_peer("peer-a".into()),
            TaskMergeJournalPageQuery::new("peer-a".into(), 1, 2, 1),
        )
        .await
        .unwrap();
    assert_eq!(next.entries[0].operation.id, id('2'));

    let mismatch = handle
        .query(
            CommandContext::authenticated_peer("peer-b".into()),
            TaskMergeJournalPageQuery::new("peer-a".into(), 0, 1, 1),
        )
        .await;
    assert!(matches!(mismatch, Err(DbError::Permission(_))));
    shutdown(&mut owner).await;
}
