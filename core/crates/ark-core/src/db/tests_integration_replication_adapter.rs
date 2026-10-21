#[tokio::test]
async fn signed_integration_adapter_uses_queries_and_transactional_commands() {
    use crate::replication_storage::IntegrationReplicationBackend;

    let database =
        ark_store::test_support::TestDatabase::create().expect("test database initializes");
    let origin_key = SigningKey::from_bytes(&[71; 32]);
    let recipient_key = SigningKey::from_bytes(&[72; 32]);
    ark_store::test_support::IntegrationReplicationTestFixture::seed_authorized_pair(
        database.path(),
        &crate::integration_replication::encode_hex(origin_key.verifying_key().as_bytes()),
        &crate::integration_replication::encode_hex(recipient_key.verifying_key().as_bytes()),
        Some("transport-a"),
        Some("transport-b"),
    )
    .unwrap();
    let backend = SqliteStorageBackend::open_for_test(
        database.path().to_string_lossy().into_owned(),
        "device-local",
    )
    .await
    .unwrap();
    admit_test_registry_peer(&backend, "node-a").await;
    let change = Change {
        entity: Entity::IntegrationConfiguration(integration_config(serde_json::json!({
            "display": {"region": "eu"}
        }))),
        vector_hlc: "2026-08-31T00:00:00.000Z:000003:node-a".into(),
    };
    let frame = sign_replication_batch(&origin_key, 2, "message-adapter", vec![change]);

    backend
        .validate_outbound_signed_integration_frame(&frame, "space-a", "node-a")
        .await
        .unwrap();
    backend
        .validate_outbound_signed_integration_frame_with_transport(
            &frame,
            "space-a",
            "node-a",
            "transport-b",
        )
        .await
        .unwrap();
    assert!(backend
        .validate_outbound_signed_integration_frame_with_transport(
            &frame,
            "space-a",
            "node-a",
            "stale-transport",
        )
        .await
        .unwrap_err()
        .contains("recipient transport identity is no longer authorized"));
    assert!(backend
        .apply_signed_integration_frame_with_transport(
            &frame,
            "space-a",
            "node-a",
            "node-b",
            Some("stale-transport"),
        )
        .await
        .unwrap_err()
        .contains("signed sync transport identity does not match the authorized node"));
    backend
        .apply_signed_integration_frame_with_transport(
            &frame,
            "space-a",
            "node-a",
            "node-b",
            Some("transport-a"),
        )
        .await
        .unwrap();
    let mut next_configuration = integration_config(serde_json::json!({
        "display": {"region": "us"}
    }));
    next_configuration.revision = 2;
    next_configuration.hlc = "2026-08-31T00:00:00.000Z:000004:node-a".into();
    let next_frame = sign_replication_batch(
        &origin_key,
        2,
        "message-adapter-next",
        vec![Change {
            entity: Entity::IntegrationConfiguration(next_configuration),
            vector_hlc: "2026-08-31T00:00:00.000Z:000004:node-a".into(),
        }],
    );
    backend
        .apply_signed_integration_frame(&next_frame, "space-a", "node-a", "node-b")
        .await
        .unwrap();

    assert!(
        ark_store::test_support::SyncTestState::connection_is_autocommit(database.path()).unwrap()
    );
    assert_eq!(
        ark_store::test_support::IntegrationReplicationTestFixture::load_configuration(
            database.path(),
            "integration-a",
        )
        .unwrap()
        .unwrap()
        .public_settings,
        serde_json::json!({"display": {"region": "us"}})
    );
    let vector =
        ark_store::test_support::SyncTestState::load_version_vector(database.path()).unwrap();
    assert_eq!(
        vector.get("integration_configuration:integration-a"),
        Some(&"2026-08-31T00:00:00.000Z:000004:node-a".to_string())
    );
}

#[tokio::test]
async fn signed_integration_command_binds_context_peer_inside_transaction() {
    let database =
        ark_store::test_support::TestDatabase::create().expect("test database initializes");
    let (db, _owner) = DbHandle::open(
        database.path().to_string_lossy().into_owned(),
        "device-local".into(),
    )
    .await
    .unwrap();
    let fixture_backend = SqliteStorageBackend::with_db_handle(db.clone());
    admit_test_registry_peer(&fixture_backend, "node-b").await;
    let signing_key = SigningKey::from_bytes(&[81; 32]);
    let frame = sign_replication_batch(&signing_key, 1, "message-wrong-peer", Vec::new());

    let error = db
        .submit_signed_integration_frame(
            CommandContext::authenticated_peer("node-b".into()),
            ApplySignedIntegrationFrameCommand::new(
                frame.clone(),
                "space-a".into(),
                "node-b".into(),
                None,
                IdentityErrorStyle::Detailed,
            ),
        )
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "signed integration frame origin is not the authenticated peer"
    );
    let generic_error = db
        .submit_signed_integration_frame(
            CommandContext::authenticated_peer("node-b".into()),
            ApplySignedIntegrationFrameCommand::new(
                frame,
                "space-a".into(),
                "node-b".into(),
                None,
                IdentityErrorStyle::Generic,
            ),
        )
        .await
        .unwrap_err();
    assert_eq!(
        generic_error.to_string(),
        "signed integration frame identity mismatch"
    );
    assert!(
        ark_store::test_support::SyncTestState::connection_is_autocommit(database.path()).unwrap()
    );
}

#[tokio::test]
async fn signed_integration_command_rolls_back_replay_and_earlier_changes() {
    use crate::replication_storage::IntegrationReplicationBackend;

    let database =
        ark_store::test_support::TestDatabase::create().expect("test database initializes");
    let origin_key = SigningKey::from_bytes(&[91; 32]);
    let recipient_key = SigningKey::from_bytes(&[92; 32]);
    ark_store::test_support::IntegrationReplicationTestFixture::seed_authorized_pair(
        database.path(),
        &crate::integration_replication::encode_hex(origin_key.verifying_key().as_bytes()),
        &crate::integration_replication::encode_hex(recipient_key.verifying_key().as_bytes()),
        None,
        None,
    )
    .unwrap();
    let backend = SqliteStorageBackend::open_for_test(
        database.path().to_string_lossy().into_owned(),
        "device-local",
    )
    .await
    .unwrap();
    admit_test_registry_peer(&backend, "node-a").await;
    let integration_id = "integration-a";
    let mut configuration = integration_config(serde_json::json!({"atomic": true}));
    configuration.integration_id = integration_id.into();
    let frame = sign_replication_batch(
        &origin_key,
        2,
        "message-atomic-rollback",
        vec![
            Change {
                entity: Entity::IntegrationConfiguration(configuration.clone()),
                vector_hlc: "2026-08-31T00:00:00.000Z:000003:node-a".into(),
            },
            Change {
                entity: Entity::IntegrationRefreshLease(
                    crate::integration_replication::IntegrationRefreshLease {
                        integration_id: integration_id.into(),
                        credential_generation: 1,
                        holder_node_id: "node-b".into(),
                        fencing_token: 1,
                        issued_at_ms: 1,
                        expires_at_ms: 2,
                    },
                ),
                vector_hlc: "2026-08-31T00:00:00.000Z:000004:node-a".into(),
            },
        ],
    );

    let error = backend
        .apply_signed_integration_frame(&frame, "space-a", "node-a", "node-b")
        .await
        .unwrap_err();
    assert!(error.contains("refresh lease holder does not match signed origin"));
    assert!(
        ark_store::test_support::IntegrationReplicationTestFixture::load_configuration(
            database.path(),
            integration_id,
        )
        .unwrap()
        .is_none()
    );
    assert!(
        !ark_store::test_support::SyncTestState::load_version_vector(database.path())
            .unwrap()
            .contains_key(&format!("integration_configuration:{integration_id}"))
    );

    let corrected_frame = sign_replication_batch(
        &origin_key,
        2,
        &frame.message_id,
        vec![Change {
            entity: Entity::IntegrationConfiguration(configuration),
            vector_hlc: "2026-08-31T00:00:00.000Z:000003:node-a".into(),
        }],
    );
    backend
        .apply_signed_integration_frame(&corrected_frame, "space-a", "node-a", "node-b")
        .await
        .unwrap();
}
