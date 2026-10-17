use super::*;

fn profile_rule(resource_id: String) -> ark_application::sync_policy::SyncProfileRule {
    ark_application::sync_policy::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id,
        mode: ark_domain::selective_sync::SyncMode::Full,
        filter: ark_domain::selective_sync::FilterV1::match_all(),
    }
}

#[tokio::test]
async fn accepted_peer_policy_is_bound_to_authenticated_actor_peer() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("accepted-peer-policy.db");
    let (handle, mut owner) = DbHandle::open(
        database_path.to_string_lossy().into_owned(),
        "local-device".into(),
    )
    .await
    .unwrap();
    let rules = vec![profile_rule("type-books".into())];
    let profile = ark_application::sync_policy::SyncProfile {
        profile_id: "remote-policy".into(),
        device_id: "phone".into(),
        revision: 4,
        name: "Phone selection".into(),
        active: true,
        rules: rules.clone(),
    };
    let digest = profile.digest();
    let denied = handle
        .submit(
            CommandContext::sync_peer("tablet".into(), super::super::CommandClass::SyncLive),
            AcceptPeerSyncPolicyCommand::new(
                "phone".into(),
                4,
                digest.clone(),
                true,
                rules.clone(),
            ),
        )
        .await;
    assert!(matches!(denied, Err(DbError::Permission(_))));

    let accepted = handle
        .submit(
            CommandContext::sync_peer("phone".into(), super::super::CommandClass::SyncLive),
            AcceptPeerSyncPolicyCommand::new("phone".into(), 4, digest, true, rules),
        )
        .await
        .unwrap();
    assert_eq!(accepted.peer_device_id, "phone");
    let persisted = handle
        .query(
            CommandContext::sync_peer("phone".into(), super::super::CommandClass::SyncLive),
            LoadAcceptedPeerSyncPolicyQuery::new("phone".into()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.revision, 4);
    assert_eq!(persisted.rules[0].resource_id, "type-books");
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn active_sync_profiles_are_device_owned_revisioned_and_durable() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("sync-policy.db");
    let (handle, mut owner) = DbHandle::open(
        database_path.to_string_lossy().into_owned(),
        "device-0".into(),
    )
    .await
    .unwrap();

    for index in 0..6 {
        let device_id = format!("device-{index}");
        let profile = handle
            .submit(
                CommandContext::engine(),
                SaveLocalSyncProfileCommand::new(
                    device_id.clone(),
                    format!("profile-{index}"),
                    format!("Device {index}"),
                    true,
                    None,
                    vec![profile_rule(format!("type-{index}"))],
                ),
            )
            .await
            .unwrap();
        assert_eq!(profile.revision, 1);
        assert_eq!(profile.device_id, device_id);
    }

    let device_three = handle
        .query(
            CommandContext::engine(),
            LoadActiveLocalSyncProfileQuery::new("device-3".into()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(device_three.rules[0].resource_id, "type-3");
    let failed_activation = handle
        .submit(
            CommandContext::engine(),
            SaveLocalSyncProfileCommand::new(
                "device-3".into(),
                "x".repeat(129),
                "Invalid profile id".into(),
                true,
                Some(1),
                vec![],
            ),
        )
        .await;
    assert!(failed_activation.is_err());
    assert_eq!(
        handle
            .query(
                CommandContext::engine(),
                LoadActiveLocalSyncProfileQuery::new("device-3".into()),
            )
            .await
            .unwrap()
            .unwrap()
            .profile_id,
        "profile-3",
        "failed policy replacement must preserve the old active profile"
    );
    assert_eq!(
        handle
            .query(
                CommandContext::engine(),
                LoadActiveLocalSyncProfileQuery::new("unconfigured-device".into()),
            )
            .await
            .unwrap(),
        None
    );

    let updated = handle
        .submit(
            CommandContext::engine(),
            SaveLocalSyncProfileCommand::new(
                "device-3".into(),
                "profile-3".into(),
                "Device Three".into(),
                true,
                Some(1),
                vec![profile_rule("updated-type".into())],
            ),
        )
        .await
        .unwrap();
    assert_eq!(updated.revision, 2);
    assert!(matches!(
        handle
            .submit(
                CommandContext::engine(),
                SaveLocalSyncProfileCommand::new(
                    "device-3".into(),
                    "profile-3".into(),
                    "stale".into(),
                    true,
                    Some(1),
                    vec![profile_rule("must-not-commit".into())],
                ),
            )
            .await,
        Err(DbError::Conflict(message)) if message == "PROFILE_REVISION_STALE"
    ));

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
    let (reopened, mut reopened_owner) = DbHandle::open(
        database_path.to_string_lossy().into_owned(),
        "device-0".into(),
    )
    .await
    .unwrap();
    let persisted = reopened
        .query(
            CommandContext::engine(),
            LoadActiveLocalSyncProfileQuery::new("device-3".into()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.revision, 2);
    assert_eq!(persisted.rules[0].resource_id, "updated-type");
    reopened_owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}
