use std::collections::HashMap;
use std::future::Future;

use ark_application::sync_storage::{
    CommittedFeedPort, PeerProgressPort, PeerSyncPolicyPort, PolicyBoundSnapshotReadError,
    PolicyIdentity, SyncEntityReadPort, SyncReadClass,
};
use ark_domain::types::{SyncEntity, VersionVector};

use crate::protocol::{
    generate_id, is_usage_entity, merge_usage_cursors, merge_vector_entries, should_send_entity,
    split_into_batches, LanSyncMessage,
};

const SNAPSHOT_PAGE_SIZE: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingSnapshotAck {
    pub batch_id: String,
    pub policy_identity: Option<PolicyIdentity>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum OutboundSnapshotProgress {
    #[default]
    NotSent,
    AwaitingFirstAck(PendingSnapshotAck),
    Acknowledged {
        pending: Option<PendingSnapshotAck>,
    },
}

impl OutboundSnapshotProgress {
    pub fn not_sent() -> Self {
        Self::NotSent
    }

    pub fn acknowledged() -> Self {
        Self::Acknowledged { pending: None }
    }

    pub fn acknowledged_awaiting_ack(pending: PendingSnapshotAck) -> Self {
        Self::Acknowledged {
            pending: Some(pending),
        }
    }

    pub fn awaiting_first_ack(pending: PendingSnapshotAck) -> Self {
        Self::AwaitingFirstAck(pending)
    }

    pub fn has_been_sent(&self) -> bool {
        !matches!(self, Self::NotSent)
    }

    pub fn has_been_acknowledged(&self) -> bool {
        matches!(self, Self::Acknowledged { .. })
    }

    pub fn pending_ack(&self) -> Option<&PendingSnapshotAck> {
        match self {
            Self::NotSent => None,
            Self::AwaitingFirstAck(pending) => Some(pending),
            Self::Acknowledged { pending } => pending.as_ref(),
        }
    }

    pub fn record_sent(&mut self, pending: PendingSnapshotAck) {
        *self = match self {
            Self::NotSent => Self::AwaitingFirstAck(pending),
            Self::AwaitingFirstAck(_) => Self::AwaitingFirstAck(pending),
            Self::Acknowledged { .. } => Self::Acknowledged {
                pending: Some(pending),
            },
        };
    }

    pub fn record_acknowledged(&mut self, expected: &PendingSnapshotAck) -> bool {
        match self {
            Self::AwaitingFirstAck(pending) if pending == expected => {
                *self = Self::acknowledged();
                true
            }
            Self::Acknowledged {
                pending: Some(pending),
            } if pending == expected => {
                *self = Self::Acknowledged { pending: None };
                true
            }
            _ => false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::NotSent;
    }
}

pub enum SnapshotEmission {
    Page(Vec<LanSyncMessage>),
    Terminal {
        ack: PendingSnapshotAck,
        message: Box<LanSyncMessage>,
    },
}

pub struct OutboundSnapshot {
    peer_device_id: String,
    remote_vector: VersionVector,
    read_class: SyncReadClass,
    complete_through: HashMap<String, u64>,
    policy_bound: bool,
    policy_identity: Option<PolicyIdentity>,
    initial_local_vector: VersionVector,
    local_vector: VersionVector,
    offset: usize,
    vector_updated: bool,
}

impl OutboundSnapshot {
    pub async fn begin<S>(
        storage: &S,
        peer_device_id: impl Into<String>,
        remote_vector: VersionVector,
        sync_complete: bool,
        policy_bound: bool,
    ) -> Result<Self, String>
    where
        S: SyncEntityReadPort + PeerProgressPort + PeerSyncPolicyPort + ?Sized,
    {
        let peer_device_id = peer_device_id.into();
        let policy_identity = if policy_bound {
            Some(
                storage
                    .accepted_policy_identity_for_peer(&peer_device_id)
                    .await?
                    .ok_or_else(|| "accepted_sync_policy_identity_missing".to_owned())?,
            )
        } else {
            None
        };
        let local_vector = storage.load_version_vector().await?;
        let read_class = SyncReadClass::for_sync_state(sync_complete);
        let complete_through = storage
            .usage_complete_through_for_peer(&peer_device_id, read_class)
            .await?;

        Ok(Self {
            peer_device_id,
            remote_vector,
            read_class,
            complete_through,
            policy_bound,
            policy_identity,
            initial_local_vector: local_vector.clone(),
            local_vector,
            offset: 0,
            vector_updated: false,
        })
    }

    pub async fn next_page<S>(&mut self, storage: &S) -> Result<SnapshotEmission, String>
    where
        S: SyncEntityReadPort + PeerProgressPort + PeerSyncPolicyPort + ?Sized,
    {
        loop {
            self.refresh_policy_identity(storage).await?;

            let mut load_vector = self.local_vector.clone();
            merge_usage_cursors(&mut load_vector, &self.remote_vector);
            let entities = match self.policy_identity.as_ref() {
                Some(identity) => match storage
                    .load_policy_bound_entities_page_for_peer(
                        &self.peer_device_id,
                        self.read_class,
                        &load_vector,
                        self.offset,
                        SNAPSHOT_PAGE_SIZE,
                        identity,
                    )
                    .await
                {
                    Ok(page) => {
                        debug_assert_eq!(page.identity, *identity);
                        page.entities
                    }
                    Err(PolicyBoundSnapshotReadError::StalePolicy) => continue,
                    Err(PolicyBoundSnapshotReadError::Storage(error)) => return Err(error),
                },
                None => {
                    storage
                        .load_entities_page_for_peer(
                            &self.peer_device_id,
                            self.read_class,
                            &load_vector,
                            self.offset,
                            SNAPSHOT_PAGE_SIZE,
                        )
                        .await?
                }
            };

            if entities.is_empty() {
                if self.refresh_policy_identity(storage).await? {
                    continue;
                }
                self.persist_vector(storage).await?;
                if self.refresh_policy_identity(storage).await? {
                    continue;
                }
                let ack = PendingSnapshotAck {
                    batch_id: generate_id(),
                    policy_identity: self.policy_identity.clone(),
                };
                return Ok(SnapshotEmission::Terminal {
                    message: Box::new(LanSyncMessage::SyncChanges {
                        policy_identity: ack.policy_identity.clone(),
                        batch_id: ack.batch_id.clone(),
                        entities: Vec::new(),
                        is_last: true,
                        origin_device_id: None,
                        usage_complete_through: (!self.complete_through.is_empty())
                            .then(|| self.complete_through.clone()),
                    }),
                    ack,
                });
            }

            self.offset += entities.len();
            let mut to_send = Vec::new();
            for entity in entities {
                self.observe_entity(&entity);
                if should_send_entity(&self.remote_vector, &entity) {
                    to_send.push(entity);
                }
            }

            if self.refresh_policy_identity(storage).await? {
                continue;
            }

            let messages = split_into_batches(&to_send)
                .into_iter()
                .map(|entities| LanSyncMessage::SyncChanges {
                    policy_identity: self.policy_identity.clone(),
                    batch_id: generate_id(),
                    entities,
                    is_last: false,
                    origin_device_id: None,
                    usage_complete_through: None,
                })
                .collect();
            return Ok(SnapshotEmission::Page(messages));
        }
    }

    pub async fn drive<S, Emit, EmitFuture>(
        &mut self,
        storage: &S,
        mut emit: Emit,
    ) -> Result<PendingSnapshotAck, String>
    where
        S: SyncEntityReadPort + PeerProgressPort + PeerSyncPolicyPort + ?Sized,
        Emit: FnMut(SnapshotEmission) -> EmitFuture,
        EmitFuture: Future<Output = Result<(), String>>,
    {
        loop {
            match self.next_page(storage).await? {
                SnapshotEmission::Page(messages) => {
                    emit(SnapshotEmission::Page(messages)).await?;
                }
                SnapshotEmission::Terminal { ack, message } => {
                    emit(SnapshotEmission::Terminal {
                        ack: ack.clone(),
                        message,
                    })
                    .await?;
                    return Ok(ack);
                }
            }
        }
    }

    fn observe_entity(&mut self, entity: &SyncEntity) {
        if !is_usage_entity(entity) && !self.local_vector.contains_key(&entity.id) {
            self.local_vector
                .insert(entity.id.clone(), entity.hlc.clone());
            self.vector_updated = true;
        }
    }

    async fn refresh_policy_identity<S>(&mut self, storage: &S) -> Result<bool, String>
    where
        S: PeerSyncPolicyPort + ?Sized,
    {
        if !self.policy_bound {
            return Ok(false);
        }
        let identity = storage
            .accepted_policy_identity_for_peer(&self.peer_device_id)
            .await?
            .ok_or_else(|| "accepted_sync_policy_identity_missing".to_owned())?;
        if self.policy_identity.as_ref() == Some(&identity) {
            return Ok(false);
        }
        self.policy_identity = Some(identity);
        self.local_vector.clone_from(&self.initial_local_vector);
        self.vector_updated = false;
        self.offset = 0;
        Ok(true)
    }

    async fn persist_vector<S>(&self, storage: &S) -> Result<(), String>
    where
        S: PeerProgressPort + ?Sized,
    {
        if !self.vector_updated {
            return Ok(());
        }
        let mut merged = storage.load_version_vector().await?;
        merge_vector_entries(&mut merged, &self.local_vector);
        storage.store_version_vector(&merged).await
    }
}

pub async fn acknowledge_snapshot<S>(
    storage: &S,
    peer_device_id: &str,
    pending: &PendingSnapshotAck,
    batch_id: &str,
    policy_identity: Option<&PolicyIdentity>,
) -> Result<bool, String>
where
    S: CommittedFeedPort + ?Sized,
{
    if pending.batch_id != batch_id || pending.policy_identity.as_ref() != policy_identity {
        return Ok(false);
    }
    match pending.policy_identity.as_ref() {
        Some(identity) => {
            storage
                .complete_outbound_snapshot_identity(peer_device_id, identity)
                .await?;
        }
        None => {
            let generation = storage
                .selective_sync_policy_generation_for_peer(peer_device_id)
                .await?;
            storage
                .complete_outbound_snapshot(peer_device_id, generation)
                .await?;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod progress_tests {
    use super::{OutboundSnapshotProgress, PendingSnapshotAck};

    fn pending(batch_id: &str) -> PendingSnapshotAck {
        PendingSnapshotAck {
            batch_id: batch_id.to_owned(),
            policy_identity: None,
        }
    }

    #[test]
    fn first_ack_completes_initial_snapshot_and_wrong_ack_is_ignored() {
        let expected = pending("snapshot-1");
        let mut progress = OutboundSnapshotProgress::not_sent();

        assert!(!progress.has_been_sent());
        assert!(!progress.has_been_acknowledged());
        progress.record_sent(expected.clone());
        assert!(progress.has_been_sent());
        assert!(!progress.record_acknowledged(&pending("stale")));
        assert_eq!(progress.pending_ack(), Some(&expected));
        assert!(progress.record_acknowledged(&expected));
        assert!(progress.has_been_acknowledged());
        assert_eq!(progress.pending_ack(), None);
        assert!(!progress.record_acknowledged(&expected));
    }

    #[test]
    fn later_snapshot_ack_preserves_initial_completion_until_it_is_acked() {
        let first = pending("snapshot-1");
        let later = pending("snapshot-2");
        let mut progress = OutboundSnapshotProgress::awaiting_first_ack(first.clone());
        assert!(progress.record_acknowledged(&first));

        progress.record_sent(later.clone());
        assert!(progress.has_been_acknowledged());
        assert_eq!(progress.pending_ack(), Some(&later));
        assert!(progress.record_acknowledged(&later));
        assert!(progress.has_been_acknowledged());
        assert_eq!(progress.pending_ack(), None);

        progress.record_sent(pending("snapshot-3"));
        progress.reset();
        assert!(!progress.has_been_sent());
        assert!(!progress.has_been_acknowledged());
        assert_eq!(progress.pending_ack(), None);
    }
}

#[cfg(test)]
#[path = "outbound_snapshot_ack_tests.rs"]
mod ack_tests;
