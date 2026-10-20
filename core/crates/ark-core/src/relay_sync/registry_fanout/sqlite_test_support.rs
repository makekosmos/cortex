use super::super::*;
use crate::db::{
    CommandContext, DbHandle, DbOwner, LocalCoreTaskWrite, ObjectReadQuery,
    SaveLocalSyncProfileCommand, SqliteStorageBackend,
};
use crate::replication_storage::PeerIdentityBackend;
use crate::sync_bind::SyncBind;
use crate::types::{ArkObject, ArkObjectWrite};
use ark_application::sync_policy::SyncProfileRule;
use ark_domain::selective_sync::{FilterV1, SyncMode};
use ark_net::iroh::RelayMode;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

pub(super) const SPACE_ID: &str = "registry-sqlite-catchup-test";

pub(super) struct ActorPeer {
    pub(super) db: DbHandle,
    pub(super) storage: Arc<SqliteStorageBackend>,
    pub(super) owner: DbOwner,
    pub(super) secret: ark_net::iroh::SecretKey,
}

pub(super) async fn open_peer(db_path: &std::path::Path, device_id: &str) -> ActorPeer {
    let (db, owner) = DbHandle::open(db_path.to_string_lossy().into_owned(), device_id.to_owned())
        .await
        .expect("open production SQLite actor");
    let secret = crate::iroh_transport::load_or_generate_secret_key_via_actor(&db)
        .await
        .expect("load persistent Iroh identity");
    let storage = Arc::new(SqliteStorageBackend::with_db_handle(db.clone()));
    storage
        .set_device_id(device_id)
        .expect("set storage device identity");
    ActorPeer {
        storage,
        db,
        owner,
        secret,
    }
}

pub(super) fn relay_config(device_id: &str) -> RelaySyncConfig {
    RelaySyncConfig {
        relay_url: String::new(),
        relay_api_key: None,
        space_id: SPACE_ID.into(),
        device_id: device_id.into(),
        device_name: device_id.into(),
        auth_secret: None,
    }
}

pub(super) fn iroh_config(
    device_id: &str,
    secret_key: ark_net::iroh::SecretKey,
    peer_addr: Option<ark_net::iroh::EndpointAddr>,
) -> crate::iroh_transport::IrohConfig {
    crate::iroh_transport::IrohConfig {
        device_id: device_id.into(),
        device_name: device_id.into(),
        space_id: SPACE_ID.into(),
        secret_key: Some(secret_key),
        peer_addr,
        peer_ticket: None,
        relay_mode: Some(RelayMode::Disabled),
        auth_secret: None,
        bind: SyncBind::Loopback,
    }
}

pub(super) async fn wait_until<F, Fut>(description: &str, mut predicate: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while !predicate().await {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {description}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

pub(super) fn task_rule(mode: SyncMode) -> SyncProfileRule {
    SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "core.task".into(),
        mode,
        filter: FilterV1::match_all(),
    }
}

pub(super) async fn set_task_policy(
    peer: &ActorPeer,
    mode: SyncMode,
    expected_revision: Option<i64>,
) {
    peer.db
        .submit(
            CommandContext::engine(),
            SaveLocalSyncProfileCommand::new(
                peer.storage.device_id(),
                "device-policy".into(),
                "Device sync policy".into(),
                true,
                expected_revision,
                vec![task_rule(mode)],
            ),
        )
        .await
        .expect("save device sync policy");
}

pub(super) async fn task_title(peer: &ActorPeer, id: &str) -> Option<String> {
    let value = peer
        .db
        .query_with_store(
            CommandContext::engine(),
            ObjectReadQuery::Get(id.to_owned()),
        )
        .await
        .expect("read task object");
    serde_json::from_value::<Option<ArkObject>>(value)
        .expect("decode task object")
        .map(|task| task.title)
}

pub(super) async fn write_task(peer: &ActorPeer, id: &str, title: &str) {
    let existing = task_title(peer, id).await.is_some();
    let expected_snapshot = if existing {
        let value = peer
            .db
            .query_with_store(
                CommandContext::engine(),
                ObjectReadQuery::WriteSnapshot(id.to_owned()),
            )
            .await
            .expect("read task write snapshot");
        Some(
            serde_json::from_value::<ark_domain::object_write::ObjectWriteSnapshot>(value)
                .expect("decode task write snapshot"),
        )
    } else {
        None
    };
    peer.db
        .submit_local_core_task_write(
            CommandContext::engine(),
            None,
            LocalCoreTaskWrite::new(
                ArkObjectWrite {
                    id: id.into(),
                    type_id: "core.task".into(),
                    type_version: Some("0.1.0".into()),
                    title: title.into(),
                    content_json: serde_json::json!({
                        "type": "doc",
                        "content": [{ "type": "paragraph" }]
                    }),
                    props_json: serde_json::json!({
                        "state": { "kind": "open", "at": null },
                        "planning": { "kind": "inbox", "period": "any", "date": null },
                        "deadline": null,
                        "reminders": [],
                        "recurrence": null,
                        "priority": "none",
                        "checklist": [],
                        "extensions": {}
                    }),
                    created_at: "2026-10-17T00:00:00.000Z".into(),
                    updated_at: "2026-10-17T00:00:00.000Z".into(),
                    deleted_at: None,
                },
                expected_snapshot,
                Some(peer.storage.device_id()),
            ),
        )
        .await
        .expect("write canonical task");
}

pub(super) async fn wait_for_accepted_task_mode(
    peer: &ActorPeer,
    remote: &str,
    revision: i64,
    mode: SyncMode,
) {
    wait_until("accepted core.task policy", || async {
        peer.storage
            .load_accepted_peer_sync_policy(remote)
            .await
            .is_ok_and(|accepted| {
                accepted.is_some_and(|policy| {
                    policy.revision == revision
                        && policy.rules.iter().any(|rule| {
                            rule.resource_kind == "type"
                                && rule.resource_id == "core.task"
                                && rule.mode == mode
                        })
                })
            })
    })
    .await;
}

pub(super) fn policy_relay_config(device_id: &str) -> RelaySyncConfig {
    let mut config = relay_config(device_id);
    config.auth_secret = Some("sqlite-policy-iroh-test-secret".into());
    config
}

pub(super) fn policy_iroh_config(
    device_id: &str,
    secret: ark_net::iroh::SecretKey,
    peer_addr: Option<ark_net::iroh::EndpointAddr>,
) -> crate::iroh_transport::IrohConfig {
    let mut config = iroh_config(device_id, secret, peer_addr);
    config.auth_secret = Some("sqlite-policy-iroh-test-secret".into());
    config
}

pub(super) fn start_policy_relay(
    peer: &ActorPeer,
    device_id: &str,
    peer_addr: Option<ark_net::iroh::EndpointAddr>,
) -> (Arc<crate::iroh_transport::IrohTransport>, Arc<RelaySync>) {
    let transport = Arc::new(crate::iroh_transport::IrohTransport::new(
        policy_iroh_config(device_id, peer.secret.clone(), peer_addr),
    ));
    let relay = RelaySync::with_iroh_transport(
        peer.storage.clone() as Arc<dyn SyncRuntimeStorage>,
        policy_relay_config(device_id),
        transport.clone(),
    )
    .expect("Iroh RelaySync construction");
    (transport, relay)
}
