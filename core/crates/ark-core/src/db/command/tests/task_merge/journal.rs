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
            TaskMergeJournalPageQuery::new(
                "peer-a".into(),
                crate::data_platform::default_sync_policy_identity(),
                0,
                1,
                1,
            ),
        )
        .await
        .unwrap();
    assert_eq!(page.page.through_position, 1);
    assert_eq!(page.page.entries.len(), 1);
    assert_eq!(page.page.entries[0].operation.id, id('1'));

    let next = handle
        .query(
            CommandContext::authenticated_peer("peer-a".into()),
            TaskMergeJournalPageQuery::new(
                "peer-a".into(),
                crate::data_platform::default_sync_policy_identity(),
                1,
                2,
                1,
            ),
        )
        .await
        .unwrap();
    assert_eq!(next.page.entries[0].operation.id, id('2'));

    let mismatch = handle
        .query(
            CommandContext::authenticated_peer("peer-b".into()),
            TaskMergeJournalPageQuery::new(
                "peer-a".into(),
                crate::data_platform::default_sync_policy_identity(),
                0,
                1,
                1,
            ),
        )
        .await;
    assert!(matches!(mismatch, Err(DbError::Permission(_))));
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn journal_page_requires_matching_policy_and_full_task_selection() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    append(&handle, operation('1', &[], "first"), None)
        .await
        .unwrap();
    let limited_policy = crate::data_platform::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "core.task".into(),
        mode: crate::data_platform::SyncMode::Metadata,
        filter: crate::data_platform::FilterV1::match_all(),
    };
    let digest = crate::data_platform::digest_sync_policy(std::slice::from_ref(&limited_policy));
    handle.with_connection(|connection| {
        crate::data_platform::accept_peer_sync_policy(
            connection,
            "peer-a",
            1,
            &digest,
            true,
            std::slice::from_ref(&limited_policy),
        )
        .unwrap();
    });
    let identity = crate::application::sync_storage::PolicyIdentity {
        revision: 1,
        digest,
        active: true,
    };
    let denied = handle
        .query(
            CommandContext::authenticated_peer("peer-a".into()),
            TaskMergeJournalPageQuery::new("peer-a".into(), identity.clone(), 0, 1, 1),
        )
        .await;
    assert!(matches!(denied, Err(DbError::Permission(_))));

    let stale = handle
        .query(
            CommandContext::authenticated_peer("peer-a".into()),
            TaskMergeJournalPageQuery::new(
                "peer-a".into(),
                crate::data_platform::default_sync_policy_identity(),
                0,
                1,
                1,
            ),
        )
        .await;
    assert!(matches!(stale, Err(DbError::Permission(_))));

    let filtered_policy = crate::data_platform::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "core.task".into(),
        mode: crate::data_platform::SyncMode::Full,
        filter: crate::data_platform::FilterV1::parse(
            r#"{"version":1,"where":{"eq":{"field":"title","value":"visible"}}}"#,
        )
        .unwrap(),
    };
    let filtered_digest =
        crate::data_platform::digest_sync_policy(std::slice::from_ref(&filtered_policy));
    handle.with_connection(|connection| {
        crate::data_platform::accept_peer_sync_policy(
            connection,
            "peer-a",
            2,
            &filtered_digest,
            true,
            std::slice::from_ref(&filtered_policy),
        )
        .unwrap();
    });
    let filtered = handle
        .query(
            CommandContext::authenticated_peer("peer-a".into()),
            TaskMergeJournalPageQuery::new(
                "peer-a".into(),
                crate::application::sync_storage::PolicyIdentity {
                    revision: 2,
                    digest: filtered_digest,
                    active: true,
                },
                0,
                1,
                1,
            ),
        )
        .await;
    assert!(matches!(filtered, Err(DbError::Permission(_))));
    shutdown(&mut owner).await;
}
