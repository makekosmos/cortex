use ark_application::sync_policy::PolicyIdentity;
use ark_application::sync_storage::PeerSyncPolicyPort;
use ark_application::task_merge::{TaskMergeBatch, TaskMergeDeliveryPort, TaskMergePrepareError};

const OUTBOUND_BATCH_LIMIT: usize = 100;

pub async fn prepare_outbound_batch<S: PeerSyncPolicyPort + TaskMergeDeliveryPort + ?Sized>(
    storage: &S,
    peer_device_id: &str,
) -> Result<Option<TaskMergeBatch>, String> {
    let Some(identity) = storage
        .accepted_policy_identity_for_peer(peer_device_id)
        .await?
    else {
        return Ok(None);
    };

    let batch = match storage
        .prepare_task_merge_batch(peer_device_id, &identity, OUTBOUND_BATCH_LIMIT)
        .await
    {
        Ok(batch) => batch,
        Err(TaskMergePrepareError::PolicyRejected) => return Ok(None),
        Err(TaskMergePrepareError::Failure(error)) => return Err(error),
    };
    let Some(batch) = batch else {
        return Ok(None);
    };

    if batch.identity != identity
        || storage
            .accepted_policy_identity_for_peer(peer_device_id)
            .await?
            .as_ref()
            != Some(&identity)
    {
        return Ok(None);
    }

    Ok(Some(batch))
}

pub async fn acknowledge_outbound_batch<S: PeerSyncPolicyPort + TaskMergeDeliveryPort + ?Sized>(
    storage: &S,
    peer_device_id: &str,
    expected_identity: &PolicyIdentity,
    batch_id: &str,
    through_position: i64,
) -> Result<bool, String> {
    if storage
        .accepted_policy_identity_for_peer(peer_device_id)
        .await?
        .as_ref()
        != Some(expected_identity)
    {
        return Ok(false);
    }

    storage
        .acknowledge_task_merge_batch(
            peer_device_id,
            expected_identity,
            batch_id,
            through_position,
        )
        .await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{acknowledge_outbound_batch, prepare_outbound_batch};
    use ark_application::{
        sync_policy::PolicyIdentity,
        sync_storage::PeerSyncPolicyPort,
        task_merge::{
            TaskMergeBatch, TaskMergeBatchAck, TaskMergeDeliveryPort, TaskMergePrepareError,
        },
    };
    use std::sync::Mutex;

    struct FakeStorage {
        identity: Mutex<Option<PolicyIdentity>>,
        rotate_identity_during_prepare: bool,
        reject_policy: bool,
        acknowledged: Mutex<Vec<(String, i64)>>,
        prepared_limit: Mutex<Option<usize>>,
    }

    impl FakeStorage {
        fn new() -> Self {
            Self {
                identity: Mutex::new(Some(identity(1))),
                rotate_identity_during_prepare: false,
                reject_policy: false,
                acknowledged: Mutex::new(Vec::new()),
                prepared_limit: Mutex::new(None),
            }
        }
    }

    #[async_trait::async_trait]
    impl PeerSyncPolicyPort for FakeStorage {
        async fn accepted_policy_identity_for_peer(
            &self,
            _peer_device_id: &str,
        ) -> Result<Option<PolicyIdentity>, String> {
            Ok(self.identity.lock().unwrap().clone())
        }
    }

    #[async_trait::async_trait]
    impl TaskMergeDeliveryPort for FakeStorage {
        async fn prepare_task_merge_batch(
            &self,
            _peer_device_id: &str,
            expected_identity: &PolicyIdentity,
            limit: usize,
        ) -> Result<Option<TaskMergeBatch>, TaskMergePrepareError> {
            *self.prepared_limit.lock().unwrap() = Some(limit);
            if self.reject_policy {
                return Err(TaskMergePrepareError::PolicyRejected);
            }
            if self.rotate_identity_during_prepare {
                *self.identity.lock().unwrap() = Some(identity(2));
            }
            Ok(Some(TaskMergeBatch {
                batch_id: "batch-1".into(),
                identity: expected_identity.clone(),
                after_position: 0,
                through_position: 1,
                entries: Vec::new(),
            }))
        }

        async fn acknowledge_task_merge_batch(
            &self,
            _peer_device_id: &str,
            _expected_identity: &PolicyIdentity,
            batch_id: &str,
            through_position: i64,
        ) -> Result<i64, String> {
            self.acknowledged
                .lock()
                .unwrap()
                .push((batch_id.to_owned(), through_position));
            Ok(through_position)
        }

        async fn apply_task_merge_batch(
            &self,
            _peer_device_id: &str,
            _local_device_id: &str,
            _expected_identity: &PolicyIdentity,
            _batch: &TaskMergeBatch,
        ) -> Result<TaskMergeBatchAck, String> {
            Err("unsupported_in_test".into())
        }
    }

    fn identity(revision: i64) -> PolicyIdentity {
        PolicyIdentity {
            revision,
            digest: format!("{revision:064x}"),
            active: true,
        }
    }

    #[tokio::test]
    async fn prepares_only_while_policy_identity_stays_current() {
        let mut storage = FakeStorage::new();
        storage.rotate_identity_during_prepare = true;

        let batch = prepare_outbound_batch(&storage, "peer").await.unwrap();

        assert!(batch.is_none());
        assert_eq!(*storage.prepared_limit.lock().unwrap(), Some(100));
    }

    #[tokio::test]
    async fn policy_refusal_is_a_no_transfer_outcome() {
        let mut storage = FakeStorage::new();
        storage.reject_policy = true;

        assert!(prepare_outbound_batch(&storage, "peer")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn acknowledgement_requires_the_current_policy_identity() {
        let storage = FakeStorage::new();

        let stale = acknowledge_outbound_batch(&storage, "peer", &identity(2), "batch-1", 1)
            .await
            .unwrap();
        let current = acknowledge_outbound_batch(&storage, "peer", &identity(1), "batch-1", 1)
            .await
            .unwrap();

        assert!(!stale);
        assert!(current);
        assert_eq!(
            *storage.acknowledged.lock().unwrap(),
            vec![("batch-1".into(), 1)]
        );
    }
}
