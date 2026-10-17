use super::*;
use crate::application::sync_storage::{CommittedFeedPort, TaskMergeDeliveryPort};
use crate::db::{
    AppendTaskMergeOperationCommand, CommandContext, DbHandle, DbOwner, EnrollPairedDeviceCommand,
    LoadOrGenerateIrohSecretKeyCommand, SqliteStorageBackend, TaskConflictSetQuery,
};
use crate::domain::task_merge::{OperationId, TaskMergeOperation, TaskMergeUnit, TaskMergeValue};
use crate::sync_server::SyncServer;
use std::time::Instant;

struct PeerFixture {
    db: DbHandle,
    owner: DbOwner,
    storage: Arc<SqliteStorageBackend>,
    secret: iroh::SecretKey,
}

async fn open_peer(path: &std::path::Path, device_id: &str) -> PeerFixture {
    let (db, owner) = DbHandle::open(path.to_string_lossy().into_owned(), device_id.into())
        .await
        .unwrap();
    let secret = db
        .submit(CommandContext::engine(), LoadOrGenerateIrohSecretKeyCommand)
        .await
        .unwrap();
    let storage = Arc::new(SqliteStorageBackend::with_db_handle(db.clone()));
    storage.set_device_id(device_id).unwrap();
    PeerFixture {
        db,
        owner,
        storage,
        secret,
    }
}

fn operation(id: char, parents: &[char], title: &str) -> TaskMergeOperation {
    let operation_id =
        |suffix| OperationId::parse(format!("{}{}", "0".repeat(31), suffix)).unwrap();
    TaskMergeOperation {
        id: operation_id(id),
        object_id: "wire-task".into(),
        unit: TaskMergeUnit::Title,
        parents: parents.iter().map(|parent| operation_id(*parent)).collect(),
        value: TaskMergeValue::Field(serde_json::Value::String(title.into())),
    }
}

#[tokio::test]
async fn authenticated_direct_sync_delivers_task_merge_batches_over_the_wire() {
    let directory = tempfile::tempdir().unwrap();
    let mut sender = open_peer(&directory.path().join("sender.sqlite"), "merge-sender").await;
    let mut receiver = open_peer(&directory.path().join("receiver.sqlite"), "merge-receiver").await;
    sender
        .db
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new(
                "merge-sender".into(),
                "merge-receiver".into(),
                receiver.secret.public().to_string(),
            ),
        )
        .await
        .unwrap();
    receiver
        .db
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new(
                "merge-receiver".into(),
                "merge-sender".into(),
                sender.secret.public().to_string(),
            ),
        )
        .await
        .unwrap();
    for (id, parents, title) in [
        ('1', vec![], "common base"),
        ('2', vec!['1'], "first edit"),
        ('3', vec!['1'], "second edit"),
    ] {
        sender
            .db
            .submit(
                CommandContext::engine(),
                AppendTaskMergeOperationCommand::new(operation(id, &parents, title), None),
            )
            .await
            .unwrap();
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let secret = "shared-task-merge-secret";
    let server =
        SyncServer::new(sender.storage.clone() as Arc<dyn crate::sync_server::SyncRuntimeStorage>);
    server.set_auth_secret(Some(secret.into())).await;
    server
        .start_with_addr(
            "task-merge-space",
            "merge-sender",
            Some("Sender"),
            None,
            &address.to_string(),
        )
        .await
        .unwrap();
    let client = SyncClient::new(
        receiver.storage.clone() as Arc<dyn crate::sync_server::SyncRuntimeStorage>,
        crate::types::PeerRecord {
            device_id: "merge-sender".into(),
            device_name: "Sender".into(),
            addresses: vec![address.to_string()],
            last_seen: String::new(),
            last_address: None,
            platform: None,
            app_version: None,
        },
        "merge-receiver".into(),
        "Receiver".into(),
        "task-merge-space".into(),
        Vec::new(),
        Some(secret.into()),
    );
    client.start();

    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            server.broadcast_task_merge_batches().await;
            let result = receiver
                .db
                .query(
                    CommandContext::engine(),
                    TaskConflictSetQuery::new("wire-task".into(), TaskMergeUnit::Title),
                )
                .await
                .unwrap();
            if result
                .conflict_set
                .is_some_and(|conflicts| conflicts.has_conflict())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("authenticated peer should receive and persist merge operations");
    let identity = sender
        .storage
        .accepted_policy_identity_for_peer("merge-receiver")
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let pending = sender
                .storage
                .prepare_task_merge_batch("merge-receiver", &identity, 100)
                .await
                .unwrap();
            if pending.is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("receiver should acknowledge the durably applied batch");
    server.stop().await;
    client.stop_and_join().await;
    for peer in [&mut sender, &mut receiver] {
        peer.owner
            .shutdown(Instant::now() + Duration::from_secs(2))
            .await
            .unwrap();
    }
}
