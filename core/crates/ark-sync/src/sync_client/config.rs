use ark_domain::types::PeerRecord;

use super::*;

pub struct DirectClientConfig {
    pub(super) peer: PeerRecord,
    pub(super) device_id: String,
    pub(super) device_name: String,
    pub(super) space_id: String,
    pub(super) own_addresses: Vec<String>,
    pub(super) auth_secret: Option<String>,
    pub(super) task_merge_bootstrap_v2: bool,
    pub(super) platform: Option<String>,
    pub(super) app_version: Option<String>,
}

impl DirectClientConfig {
    pub fn new(
        peer: PeerRecord,
        device_id: String,
        device_name: String,
        space_id: String,
        own_addresses: Vec<String>,
        auth_secret: Option<String>,
    ) -> Self {
        Self {
            peer,
            device_id,
            device_name,
            space_id,
            own_addresses,
            auth_secret,
            task_merge_bootstrap_v2: true,
            platform: None,
            app_version: None,
        }
    }

    pub fn with_platform(mut self, platform: Option<String>, app_version: Option<String>) -> Self {
        self.platform = platform;
        self.app_version = app_version;
        self
    }
}

pub(super) async fn load_version_vector(
    storage: &Arc<dyn SyncRuntimeStorage>,
) -> Result<VersionVector, String> {
    storage.load_version_vector().await
}

pub(super) async fn send_msg(tx: &ClientMessageSender, msg: LanSyncMessage) -> Result<(), String> {
    tx.send(msg).await
}
