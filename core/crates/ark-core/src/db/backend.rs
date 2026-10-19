use std::sync::{Arc, Mutex};

use super::DbHandle;

mod adapter;
mod apply;
mod legacy_mutation;
mod object_mutation;
mod signed_integration;
mod type_mutation;

pub(crate) use legacy_mutation::SqliteLegacyMutationSession;
pub(crate) use object_mutation::SqliteObjectMutationSession;
pub(crate) use signed_integration::SqliteSignedIntegrationFrameSession;
pub(crate) use type_mutation::SqliteTypeMutationSession;

pub struct SqliteStorageBackend {
    pub(in crate::db) db: DbHandle,
    device_id: Arc<Mutex<String>>,
    selective_profile: Arc<Mutex<Option<ark_domain::selective_sync::SelectiveSyncProfile>>>,
}

impl SqliteStorageBackend {
    /// Builds the raw-connection adapter used by SQLite test fixtures.
    #[cfg(debug_assertions)]
    pub fn for_test(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self::with_db_handle(DbHandle::for_test(conn))
    }

    pub(crate) fn with_db_handle(db: DbHandle) -> Self {
        Self {
            db,
            device_id: Arc::new(Mutex::new(String::new())),
            selective_profile: Arc::new(Mutex::new(None)),
        }
    }

    /// Set the device id used to stamp HLCs on entities that don't yet carry
    /// one in the version vector. Matches the TS `DelphiStorage` behaviour
    /// (`HLC.now(deviceId)` when `vector[id]` is missing).
    pub fn set_device_id(&self, device_id: &str) -> Result<(), String> {
        // Poison recovery: device_id вЂ” РїСЂРѕСЃС‚РѕР№ String, poison РЅРµРІРѕР·РјРѕР¶РµРЅ РѕС‚
        // sane code path, РЅРѕ Р·Р°С‰РёС‚Р° cheap Рё СЃРѕРіР»Р°СЃРѕРІР°РЅР° СЃ РѕСЃС‚Р°Р»СЊРЅС‹РјРё
        // SqliteStorageBackend lock'Р°РјРё (СЃРј. load_entities / apply_entity).
        let mut guard = self.device_id.lock().unwrap_or_else(|e| e.into_inner());
        *guard = device_id.to_string();
        Ok(())
    }

    pub fn device_id(&self) -> String {
        self.device_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
