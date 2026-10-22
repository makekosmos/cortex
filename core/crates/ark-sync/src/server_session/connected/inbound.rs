use super::{ConnectedServerSession, ServerSessionActor};
use ark_sync_contracts::protocol_negotiation::ProtocolCapability;
use ark_sync_contracts::session_io::SessionIoSender;
use ark_sync_contracts::wire::LanSyncMessage;

use crate::{
    inbound_live_change::{
        apply_live_change, prepare_live_change, IncomingLiveChange, LiveChangeDisposition,
        VersionVectorWriteMode,
    },
    inbound_snapshot::{InboundSnapshotBatch, SnapshotEntityResult},
    server_session::DirectServerSyncChanges,
};
use ark_application::sync_storage::entity_inbox_message_id;
use ark_domain::types::{PeerRecord, SyncEntity};

impl ConnectedServerSession {
    pub(super) async fn apply_snapshot_batch(
        &mut self,
        actor: &ServerSessionActor,
        changes: DirectServerSyncChanges,
        sender: &SessionIoSender,
    ) -> bool {
        let Some(peer_device_id) = self.peer.device_id.clone() else {
            return false;
        };
        let protocol = self.protocol.clone();
        let local_device_id = self.request.device_id.clone();
        let local_profile = match self.storage.load_local_sync_profile(&local_device_id).await {
            Ok(profile) if !actor.is_revoked() => profile,
            Ok(_) => return false,
            Err(error) => {
                eprintln!("local policy identity read failed: {error}");
                return true;
            }
        };
        let mut snapshot = match InboundSnapshotBatch::begin(
            self.storage.as_ref(),
            local_device_id,
            peer_device_id.clone(),
            protocol.as_ref(),
            local_profile.as_ref(),
            changes.policy_identity.clone(),
        )
        .await
        {
            Ok(snapshot) if !actor.is_revoked() => snapshot,
            Ok(_) => return false,
            Err(error) => {
                eprintln!("rejected snapshot page: {error}");
                return true;
            }
        };
        for entity in &changes.entities {
            if actor.is_revoked() {
                return false;
            }
            let message_id = entity_inbox_message_id(
                &peer_device_id,
                &entity.entity_type,
                &entity.id,
                &entity.hlc,
            );
            match snapshot
                .apply_entity(self.storage.as_ref(), entity, &message_id)
                .await
            {
                Ok(SnapshotEntityResult::Accepted) if !actor.is_revoked() => {
                    self.host
                        .entity_applied(&peer_device_id, entity.clone())
                        .await;
                    if actor.is_revoked() {
                        return false;
                    }
                }
                Ok(SnapshotEntityResult::Accepted) => return false,
                Ok(SnapshotEntityResult::Skipped | SnapshotEntityResult::Pending) => {}
                Err(error) => {
                    eprintln!(
                        "failed to apply sync entity {}:{}: {error}",
                        entity.entity_type, entity.id
                    );
                }
            }
        }
        if snapshot
            .finish(
                self.storage.as_ref(),
                changes.is_last,
                changes.usage_complete_through.as_ref(),
            )
            .await
            .is_err()
        {
            eprintln!("failed to persist pulled sync progress");
            return !actor.is_revoked();
        }
        if actor.is_revoked() {
            return false;
        }
        sender
            .send(LanSyncMessage::SyncAck {
                batch_id: changes.batch_id,
                accepted: snapshot.accepted_count(),
                policy_identity: changes.policy_identity,
            })
            .await
            .is_ok()
            && !actor.is_revoked()
    }

    pub(super) async fn handle_live_change(
        &mut self,
        actor: &ServerSessionActor,
        change_id: String,
        entity: SyncEntity,
        policy_identity: Option<ark_domain::sync_policy::PolicyIdentity>,
        sender: &SessionIoSender,
    ) -> bool {
        let Some(peer_device_id) = self.peer.device_id.clone() else {
            return false;
        };
        let local_device_id = self.request.device_id.clone();
        let local_profile = match self.storage.load_local_sync_profile(&local_device_id).await {
            Ok(profile) if !actor.is_revoked() => profile,
            Ok(_) => return false,
            Err(error) => {
                eprintln!("local policy identity read failed: {error}");
                return true;
            }
        };
        let notification_entity = entity.clone();
        let prepared = match prepare_live_change(
            self.storage.as_ref(),
            IncomingLiveChange {
                protocol: self.protocol.as_ref(),
                local_profile: local_profile.as_ref(),
                local_device_id: &local_device_id,
                peer_device_id: &peer_device_id,
                change_id: change_id.clone(),
                entity,
                policy_identity: policy_identity.clone(),
            },
        )
        .await
        {
            Ok(prepared) if !actor.is_revoked() => prepared,
            Ok(_) => return false,
            Err(error) => {
                eprintln!("inbound live-change preparation failed: {error}");
                return true;
            }
        };
        if prepared.receipt_recorded
            && (sender
                .send(LanSyncMessage::LiveReceiptAck {
                    policy_identity: policy_identity.clone(),
                    change_id: change_id.clone(),
                })
                .await
                .is_err()
                || actor.is_revoked())
        {
            return false;
        }
        let result = match apply_live_change(
            self.storage.as_ref(),
            prepared,
            VersionVectorWriteMode::MergeConcurrent,
        )
        .await
        {
            Ok(result) if !actor.is_revoked() => result,
            Ok(_) => return false,
            Err(error) => {
                eprintln!("failed to apply live sync entity: {error}");
                return true;
            }
        };
        match result.disposition {
            LiveChangeDisposition::Applied { notify_local } => {
                if notify_local {
                    self.host
                        .entity_applied(&peer_device_id, notification_entity)
                        .await;
                    if actor.is_revoked() {
                        return false;
                    }
                }
                sender
                    .send(LanSyncMessage::LiveAck {
                        change_id,
                        policy_identity,
                    })
                    .await
                    .is_ok()
                    && !actor.is_revoked()
            }
            LiveChangeDisposition::Stale => {
                sender
                    .send(LanSyncMessage::LiveAck {
                        change_id,
                        policy_identity,
                    })
                    .await
                    .is_ok()
                    && !actor.is_revoked()
            }
            LiveChangeDisposition::Pending => true,
        }
    }

    pub(super) async fn handle_signed_integration_frame(
        &self,
        actor: &ServerSessionActor,
        frame: crate::SignedSyncEnvelope,
        sender: &SessionIoSender,
    ) -> bool {
        let Some(authenticated_peer_id) = self.peer.device_id.as_deref() else {
            return false;
        };
        if frame.space_id != self.request.space_id
            || frame.origin_node_id != authenticated_peer_id
            || frame.recipient_node_id != self.request.device_id
        {
            return true;
        }
        if self
            .storage
            .apply_signed_integration_frame(
                &frame,
                &self.request.space_id,
                authenticated_peer_id,
                &self.request.device_id,
            )
            .await
            .is_err()
            || actor.is_revoked()
        {
            return !actor.is_revoked();
        }
        sender
            .send(LanSyncMessage::SignedIntegrationAck {
                message_id: frame.message_id,
                accepted: true,
            })
            .await
            .is_ok()
            && !actor.is_revoked()
    }

    pub(super) async fn handle_peer_list(
        &self,
        actor: &ServerSessionActor,
        incoming: Vec<PeerRecord>,
    ) -> bool {
        let removed = match self.storage.load_removed_peer_ids().await {
            Ok(removed) if !actor.is_revoked() => removed,
            Ok(_) => return false,
            Err(error) => {
                eprintln!("failed to load blocked peer list: {error}");
                return true;
            }
        };
        let filtered = filter_peer_list(
            incoming,
            &self.request.device_id,
            &self.request.addresses,
            &removed,
        );
        self.host.discovered_peers(filtered).await;
        !actor.is_revoked()
    }

    pub(super) async fn apply_task_merge_batch(
        &self,
        actor: &ServerSessionActor,
        batch: ark_domain::task_merge_sync::TaskMergeBatch,
        sender: &SessionIoSender,
    ) -> bool {
        let Some(peer_device_id) = self.peer.device_id.as_deref() else {
            return false;
        };
        let policy_bound = self.protocol.as_ref().is_some_and(|protocol| {
            protocol
                .capabilities
                .contains(&ProtocolCapability::TaskMergeBootstrapV2)
        });
        if !self.snapshot_ready() || !policy_bound {
            return true;
        }
        let ack = match self
            .storage
            .apply_task_merge_batch(
                peer_device_id,
                &self.request.device_id,
                &batch.identity,
                &batch,
            )
            .await
        {
            Ok(ack) if !actor.is_revoked() => ack,
            Ok(_) => return false,
            Err(error) => {
                eprintln!("task merge batch rejected: {error}");
                return true;
            }
        };
        if sender
            .send(LanSyncMessage::TaskMergeBatchAck {
                batch_id: ack.batch_id,
                through_position: ack.through_position,
                policy_identity: batch.identity,
            })
            .await
            .is_err()
            || actor.is_revoked()
        {
            return false;
        }
        self.host.task_merge_batch_applied().await;
        !actor.is_revoked()
    }

    pub(super) async fn handle_task_merge_ack(
        &mut self,
        actor: &ServerSessionActor,
        batch_id: String,
        through_position: i64,
        identity: ark_domain::sync_policy::PolicyIdentity,
        sender: &SessionIoSender,
    ) -> bool {
        let Some(device_id) = self.peer.device_id.as_deref() else {
            return false;
        };
        if !self.snapshot_ready()
            || !self.protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&ProtocolCapability::TaskMergeBootstrapV2)
            })
        {
            return true;
        }
        let advanced = match crate::task_merge_delivery::acknowledge_outbound_batch(
            self.storage.as_ref(),
            device_id,
            &identity,
            &batch_id,
            through_position,
        )
        .await
        {
            Ok(advanced) if !actor.is_revoked() => advanced,
            Ok(_) => return false,
            Err(error) => {
                eprintln!("task merge batch ACK failed: {error}");
                return true;
            }
        };
        !advanced || self.pump_task_merge(actor, sender).await
    }
}

fn filter_peer_list(
    incoming: Vec<PeerRecord>,
    local_device_id: &str,
    own_addresses: &[String],
    removed_peer_ids: &[String],
) -> Vec<PeerRecord> {
    incoming
        .into_iter()
        .filter(|peer| {
            peer.device_id != local_device_id
                && !removed_peer_ids.iter().any(|id| id == &peer.device_id)
                && (peer.addresses.is_empty()
                    || !peer
                        .addresses
                        .iter()
                        .all(|address| own_addresses.contains(address)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::filter_peer_list;
    use ark_domain::types::PeerRecord;

    #[test]
    fn peer_list_does_not_resurrect_removed_peers() {
        let incoming = vec![peer("blocked"), peer("fresh")];
        let filtered = filter_peer_list(incoming, "local", &[], &["blocked".into()]);
        assert_eq!(
            filtered
                .into_iter()
                .map(|peer| peer.device_id)
                .collect::<Vec<_>>(),
            vec!["fresh"]
        );
    }

    fn peer(device_id: &str) -> PeerRecord {
        PeerRecord {
            device_id: device_id.into(),
            device_name: device_id.into(),
            addresses: vec![format!("{device_id}:1")],
            last_seen: "2026-10-19T00:00:00.000Z".into(),
            last_address: None,
            platform: None,
            app_version: None,
        }
    }
}
