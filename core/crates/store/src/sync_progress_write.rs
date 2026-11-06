use domain::sync_policy::sync_profile_identity;
use domain::sync_policy::PolicyIdentity;
use domain::types::VersionVector;

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

#[derive(Debug, PartialEq, Eq)]
pub enum SyncProgressWriteError {
    MissingLocalDevice,
    StalePolicy,
    PolicyStorage(String),
    SyncValueStorage(String),
}

/// Applies one progress write inside the caller's transaction.
pub fn apply_sync_progress_write(
    transaction: &rusqlite::Transaction<'_>,
    write: SyncProgressWrite,
) -> Result<(), SyncProgressWriteError> {
    if let Some(expected) = write.policy_identity.as_ref() {
        let local_device_id = write
            .local_device_id
            .as_deref()
            .ok_or(SyncProgressWriteError::MissingLocalDevice)?;
        let profile =
            crate::data_platform::load_sync_profile_for_sync(transaction, local_device_id)
                .map_err(SyncProgressWriteError::PolicyStorage)?;
        if &sync_profile_identity(profile.as_ref()) != expected {
            return Err(SyncProgressWriteError::StalePolicy);
        }
    }
    match write.mutation {
        SyncProgressMutation::Replace { key, value } => {
            crate::sync_kv::set_sync_kv(transaction, &key, &value)
                .map_err(SyncProgressWriteError::SyncValueStorage)
        }
        SyncProgressMutation::MergeVersionVector(vector) => {
            let mut current = crate::sync_kv::get_sync_kv(transaction, crate::VERSION_VECTOR_KEY)
                .map_err(SyncProgressWriteError::SyncValueStorage)?
                .map(|raw| {
                    serde_json::from_str(&raw).map_err(|error| {
                        SyncProgressWriteError::SyncValueStorage(error.to_string())
                    })
                })
                .transpose()?
                .unwrap_or_default();
            domain::sync_storage::merge_version_vector_entries(&mut current, &vector);
            let encoded = serde_json::to_string(&current)
                .map_err(|error| SyncProgressWriteError::SyncValueStorage(error.to_string()))?;
            crate::sync_kv::set_sync_kv(transaction, crate::VERSION_VECTOR_KEY, &encoded)
                .map_err(SyncProgressWriteError::SyncValueStorage)
        }
    }
}
