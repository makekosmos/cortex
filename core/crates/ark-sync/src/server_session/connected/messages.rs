use super::{ConnectedServerSession, ServerSessionActor};
use crate::{
    protocol::{LanSyncMessage, ProtocolCapability},
    server_session::DirectServerMessage,
    session_io::SessionIoSender,
};
use ark_application::sync_storage::OutboundAck;

impl ConnectedServerSession {
    pub(in crate::server_session) async fn handle_message(
        &mut self,
        actor: &ServerSessionActor,
        message: DirectServerMessage,
        sender: &SessionIoSender,
    ) -> bool {
        if actor.is_revoked() {
            return false;
        }
        match message {
            DirectServerMessage::VersionVector(vector) => {
                self.send_snapshot(actor, vector, sender).await
            }
            DirectServerMessage::SyncChanges(changes) => {
                self.apply_snapshot_batch(actor, changes, sender).await
            }
            DirectServerMessage::Live(message) => {
                self.handle_live_message(actor, message, sender).await
            }
            DirectServerMessage::SyncPolicy(_)
            | DirectServerMessage::RegistryAdmission(_)
            | DirectServerMessage::TransportControl(_) => true,
        }
    }

    pub(super) async fn handle_live_message(
        &mut self,
        actor: &ServerSessionActor,
        message: LanSyncMessage,
        sender: &SessionIoSender,
    ) -> bool {
        match message {
            LanSyncMessage::SyncAck {
                batch_id,
                policy_identity,
                ..
            } => {
                self.handle_snapshot_ack(actor, batch_id, policy_identity, sender)
                    .await
            }
            LanSyncMessage::SignedIntegrationFrame { frame } => {
                self.handle_signed_integration_frame(actor, frame, sender)
                    .await
            }
            LanSyncMessage::SignedIntegrationAck { .. } => true,
            LanSyncMessage::LiveChange {
                change_id,
                entity,
                policy_identity,
                ..
            } => {
                self.handle_live_change(actor, change_id, entity, policy_identity, sender)
                    .await
            }
            LanSyncMessage::LiveAck {
                change_id,
                policy_identity,
            } => {
                self.handle_feed_ack(
                    actor,
                    change_id,
                    policy_identity,
                    OutboundAck::Applied,
                    sender,
                )
                .await
            }
            LanSyncMessage::LiveReceiptAck {
                change_id,
                policy_identity,
            } => {
                self.handle_feed_ack(
                    actor,
                    change_id,
                    policy_identity,
                    OutboundAck::DurableReceipt,
                    sender,
                )
                .await
            }
            LanSyncMessage::TaskMergeBatch { batch }
                if self.snapshot_ready()
                    && self.supports(ProtocolCapability::TaskMergeBootstrapV2) =>
            {
                self.apply_task_merge_batch(actor, batch, sender).await
            }
            LanSyncMessage::TaskMergeBatchAck {
                batch_id,
                through_position,
                policy_identity,
            } => {
                self.handle_task_merge_ack(
                    actor,
                    batch_id,
                    through_position,
                    policy_identity,
                    sender,
                )
                .await
            }
            LanSyncMessage::PeerList { peers } => self.handle_peer_list(actor, peers).await,
            _ => true,
        }
    }
}
