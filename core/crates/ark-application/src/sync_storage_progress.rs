use crate::sync_policy::{sync_profile_identity, SyncProfile};
use crate::sync_storage::PolicyIdentity;

#[derive(Clone, Debug, serde::Serialize)]
pub struct SyncProgressWrite {
    key: String,
    value: String,
    local_device_id: Option<String>,
    policy_identity: Option<PolicyIdentity>,
}

impl SyncProgressWrite {
    pub fn new(key: String, value: String) -> Self {
        Self {
            key,
            value,
            local_device_id: None,
            policy_identity: None,
        }
    }

    pub fn with_policy_identity(
        mut self,
        local_device_id: String,
        policy_identity: PolicyIdentity,
    ) -> Self {
        self.local_device_id = Some(local_device_id);
        self.policy_identity = Some(policy_identity);
        self
    }

    pub fn weight(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }
}

pub trait SyncProgressWriteSession {
    fn load_sync_profile_for_sync(
        &self,
        local_device_id: &str,
    ) -> Result<Option<SyncProfile>, String>;
    fn set_sync_value(&self, key: &str, value: &str) -> Result<(), String>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum SyncProgressWriteError {
    MissingLocalDevice,
    StalePolicy,
    PolicyStorage(String),
    SyncValueStorage(String),
}

pub fn apply_sync_progress_write(
    session: &dyn SyncProgressWriteSession,
    write: SyncProgressWrite,
) -> Result<(), SyncProgressWriteError> {
    if let Some(expected) = write.policy_identity.as_ref() {
        let local_device_id = write
            .local_device_id
            .as_deref()
            .ok_or(SyncProgressWriteError::MissingLocalDevice)?;
        let profile = session
            .load_sync_profile_for_sync(local_device_id)
            .map_err(SyncProgressWriteError::PolicyStorage)?;
        if &sync_profile_identity(profile.as_ref()) != expected {
            return Err(SyncProgressWriteError::StalePolicy);
        }
    }
    session
        .set_sync_value(&write.key, &write.value)
        .map_err(SyncProgressWriteError::SyncValueStorage)
}
