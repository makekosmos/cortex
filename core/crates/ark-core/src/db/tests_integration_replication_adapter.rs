#[tokio::test]
async fn signed_integration_adapter_uses_queries_and_transactional_commands() {
    use crate::replication_storage::IntegrationReplicationBackend;

    let (target, origin_key) = trusted_target(71);
    target
        .execute(
            "UPDATE authorized_nodes SET transport_public_key = ?1 WHERE node_id = ?2",
            ["transport-a", "node-a"],
        )
        .unwrap();
    target
        .execute(
            "UPDATE authorized_nodes SET transport_public_key = ?1 WHERE node_id = ?2",
            ["transport-b", "node-b"],
        )
        .unwrap();
    let connection = std::sync::Arc::new(std::sync::Mutex::new(target));
    let backend = SqliteStorageBackend::for_test(connection.clone());
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

    let guard = connection.lock().unwrap();
    assert!(guard.is_autocommit());
    assert_eq!(
        ark_store::integration_replication::load_integration_configuration(&guard, "integration-a")
            .unwrap()
            .unwrap()
            .public_settings,
        serde_json::json!({"display": {"region": "us"}})
    );
    let vector = load_version_vector(&guard).unwrap();
    assert_eq!(
        vector.get("integration_configuration:integration-a"),
        Some(&"2026-08-31T00:00:00.000Z:000004:node-a".to_string())
    );
}

#[tokio::test]
async fn signed_integration_command_binds_context_peer_inside_transaction() {
    let connection = std::sync::Arc::new(std::sync::Mutex::new(setup_db()));
    let fixture_backend = SqliteStorageBackend::for_test(connection.clone());
    admit_test_registry_peer(&fixture_backend, "node-b").await;
    let db = DbHandle::for_test(connection.clone());
    let signing_key = SigningKey::from_bytes(&[81; 32]);
    let frame = sign_replication_batch(&signing_key, 1, "message-wrong-peer", Vec::new());

    let error = db
        .submit(
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
        .submit(
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
    assert!(connection.lock().unwrap().is_autocommit());
}

#[tokio::test]
async fn signed_integration_command_rolls_back_replay_and_earlier_changes() {
    use crate::replication_storage::IntegrationReplicationBackend;

    let (target, origin_key) = trusted_target(91);
    let connection = std::sync::Arc::new(std::sync::Mutex::new(target));
    let backend = SqliteStorageBackend::for_test(connection.clone());
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
    {
        let guard = connection.lock().unwrap();
        assert!(
            ark_store::integration_replication::load_integration_configuration(
                &guard,
                integration_id
            )
            .unwrap()
            .is_none()
        );
        assert!(!load_version_vector(&guard)
            .unwrap()
            .contains_key(&format!("integration_configuration:{integration_id}")));
        let reservations: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM sync_signed_replay_reservations WHERE message_id = ?1",
                [&frame.message_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reservations, 0);
    }

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
