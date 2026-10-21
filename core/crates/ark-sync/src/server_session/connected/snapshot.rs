use super::{ConnectedServerSession, ServerSessionActor};
use ark_sync_contracts::protocol_negotiation::ProtocolCapability;
use ark_sync_contracts::session_io::SessionIoSender;

use crate::outbound_snapshot::{acknowledge_snapshot, OutboundSnapshot, SnapshotEmission};
use ark_domain::types::VersionVector;

impl ConnectedServerSession {
    pub(super) async fn send_snapshot(
        &mut self,
        actor: &ServerSessionActor,
        remote_vector: VersionVector,
        sender: &SessionIoSender,
    ) -> bool {
        let Some(device_id) = self.peer.device_id.clone() else {
            return false;
        };
        let policy_bound = self.protocol.as_ref().is_some_and(|protocol| {
            protocol
                .capabilities
                .contains(&ProtocolCapability::PolicyBoundDeliveryV1)
        });
        let mut snapshot = match OutboundSnapshot::begin(
            self.storage.as_ref(),
            device_id.clone(),
            remote_vector,
            self.snapshot_progress.has_been_acknowledged(),
            policy_bound,
        )
        .await
        {
            Ok(snapshot) if !actor.is_revoked() => snapshot,
            Ok(_) => return false,
            Err(error) => {
                eprintln!("outbound snapshot setup failed for {device_id}: {error}");
                return true;
            }
        };
        let storage = self.storage.clone();
        let result = snapshot
            .drive(storage.as_ref(), |emission| async {
                if actor.is_revoked() {
                    return Err("session_revoked".to_owned());
                }
                match emission {
                    SnapshotEmission::Page(messages) => {
                        for message in messages {
                            if sender.send_bulk_wait(message).await.is_err() || actor.is_revoked() {
                                return Err("snapshot_send_failed".to_owned());
                            }
                        }
                    }
                    SnapshotEmission::Terminal { message, .. } => {
                        if sender.send_bulk_wait(*message).await.is_err() || actor.is_revoked() {
                            return Err("snapshot_terminal_send_failed".to_owned());
                        }
                    }
                }
                Ok(())
            })
            .await;
        match result {
            Ok(pending) => {
                self.snapshot_progress.record_sent(pending);
                true
            }
            Err(error) => {
                if !actor.is_revoked() {
                    eprintln!("outbound snapshot failed for {device_id}: {error}");
                }
                !actor.is_revoked()
            }
        }
    }

    pub(super) async fn handle_snapshot_ack(
        &mut self,
        actor: &ServerSessionActor,
        batch_id: String,
        policy_identity: Option<ark_domain::sync_policy::PolicyIdentity>,
        sender: &SessionIoSender,
    ) -> bool {
        let Some(device_id) = self.peer.device_id.clone() else {
            return false;
        };
        let Some(pending) = self
            .snapshot_progress
            .pending_ack()
            .filter(|pending| pending.batch_id == batch_id)
            .cloned()
        else {
            return true;
        };
        let completion = acknowledge_snapshot(
            self.storage.as_ref(),
            &device_id,
            &pending,
            &batch_id,
            policy_identity.as_ref(),
        )
        .await;
        match completion {
            Ok(true) => {}
            Ok(false) => return !actor.is_revoked(),
            Err(error) => {
                eprintln!("outbound snapshot completion failed: {error}");
                return !actor.is_revoked();
            }
        }
        if actor.is_revoked() || !self.snapshot_progress.record_acknowledged(&pending) {
            return false;
        }
        if !self.pump_committed_feed(actor, sender).await {
            return false;
        }
        self.pump_task_merge(actor, sender).await
    }
}
