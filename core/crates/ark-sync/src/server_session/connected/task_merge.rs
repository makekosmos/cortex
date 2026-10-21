use super::{ConnectedServerSession, ServerSessionActor};
use ark_sync_contracts::protocol_negotiation::ProtocolCapability;
use ark_sync_contracts::session_io::SessionIoSender;
use ark_sync_contracts::wire::LanSyncMessage;

use crate::task_merge_delivery::prepare_outbound_batch;

impl ConnectedServerSession {
    pub(in crate::server_session) async fn pump_task_merge(
        &mut self,
        actor: &ServerSessionActor,
        sender: &SessionIoSender,
    ) -> bool {
        let Some(device_id) = self.peer.device_id.as_deref() else {
            return false;
        };
        if actor.is_revoked()
            || !self.snapshot_ready()
            || !self.supports(ProtocolCapability::TaskMergeBootstrapV2)
        {
            return !actor.is_revoked();
        }
        let batch = match prepare_outbound_batch(self.storage.as_ref(), device_id).await {
            Ok(batch) if !actor.is_revoked() => batch,
            Ok(_) => return false,
            Err(error) => {
                eprintln!("task merge batch preparation failed: {error}");
                return true;
            }
        };
        let Some(batch) = batch else {
            return true;
        };
        sender
            .send_bulk_wait(LanSyncMessage::TaskMergeBatch { batch })
            .await
            .is_ok()
            && !actor.is_revoked()
    }
}
