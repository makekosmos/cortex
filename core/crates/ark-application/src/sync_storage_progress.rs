use crate::sync_policy::{sync_profile_identity, SyncProfile};
use crate::sync_storage::PolicyIdentity;
use ark_domain::hlc::HLC;
use ark_domain::types::VersionVector;

#[derive(Clone, Debug, serde::Serialize)]
enum SyncProgressMutation {
    Replace { key: String, value: String },
    MergeVersionVector(VersionVector),
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SyncProgressWrite {
    mutation: SyncProgressMutation,
    local_device_id: Option<String>,
    policy_identity: Option<PolicyIdentity>,
}

impl SyncProgressWrite {
    pub fn new(key: String, value: String) -> Self {
        Self {
            mutation: SyncProgressMutation::Replace { key, value },
            local_device_id: None,
            policy_identity: None,
        }
    }

    pub fn merge_version_vector(vector: VersionVector) -> Self {
        Self {
            mutation: SyncProgressMutation::MergeVersionVector(vector),
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
    fn merge_version_vector(&self, vector: &VersionVector) -> Result<(), String>;
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
    match write.mutation {
        SyncProgressMutation::Replace { key, value } => session
            .set_sync_value(&key, &value)
            .map_err(SyncProgressWriteError::SyncValueStorage),
        SyncProgressMutation::MergeVersionVector(vector) => session
            .merge_version_vector(&vector)
            .map_err(SyncProgressWriteError::SyncValueStorage),
    }
}

pub fn merge_version_vector_entries(target: &mut VersionVector, source: &VersionVector) {
    for (key, value) in source {
        if key.starts_with("@usage:") {
            let regresses = value
                .parse::<u64>()
                .ok()
                .zip(
                    target
                        .get(key)
                        .and_then(|current| current.parse::<u64>().ok()),
                )
                .is_some_and(|(incoming, current)| incoming < current);
            if !regresses {
                target.insert(key.clone(), value.clone());
            }
            continue;
        }

        let outdated = target
            .get(key)
            .is_some_and(|current| !HLC::is_newer(value, current));
        if !outdated {
            target.insert(key.clone(), value.clone());
        }
    }
}
