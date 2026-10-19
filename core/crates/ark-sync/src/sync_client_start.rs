use super::*;
use ark_application::sync_storage::OutboundAck;

#[path = "sync_client_start/connect_and_receive/feed_recovery.rs"]
mod feed_recovery;
#[path = "sync_client_start/connect_and_receive/live_messages.rs"]
mod live_messages;
#[path = "sync_client_start/connect_and_receive/registry_admission.rs"]
mod registry_admission;
#[path = "sync_client_start/connect_and_receive/snapshot_ack.rs"]
mod snapshot_ack;
#[path = "sync_client_start/connect_and_receive/snapshot_drive.rs"]
mod snapshot_drive;
#[path = "sync_client_start/connect_and_receive/sync_changes.rs"]
mod sync_changes;
#[path = "sync_client_start/connect_and_receive/sync_policy.rs"]
mod sync_policy;
impl DirectClient {
    /// Start connecting to the peer. Spawns a background task.
    pub fn start(&self) {
        self.stopped
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let storage = self.storage.clone();
        let peer = self.peer.clone();
        let device_id = self.device_id.clone();
        let device_name = self.device_name.clone();
        let space_id = self.space_id.clone();
        let own_addresses = self.own_addresses.clone();
        let auth_secret = self.auth_secret.clone();
        let task_merge_bootstrap_v2 = self.task_merge_bootstrap_v2;
        let platform = self.platform.clone();
        let app_version = self.app_version.clone();
        let connector = self.connector.clone();
        let stopped = self.stopped.clone();
        let authenticated_tx = self.authenticated_tx.clone();
        let feed_in_flight = self.feed_in_flight.clone();
        let on_change = self.on_change.clone();
        let on_connected = self.on_connected.clone();
        let on_disconnected = self.on_disconnected.clone();
        let on_peer_list = self.on_peer_list.clone();
        let on_registry_events = self.on_registry_events.clone();
        let task = tokio::spawn(include!("sync_client_start/connection_loop.rs"));
        track_task(&self.tasks, task);
    }
}
