use std::sync::{Arc, Mutex};

use super::DbHandle;
use crate::db::ensure_usage_sequence_migrated;

mod adapter;
mod apply;
mod canonical_object;
mod entities;
mod object_mutation;
mod usage;

pub use apply::delete_tag;
pub(crate) use canonical_object::SqliteObjectWriteSession;
pub(crate) use object_mutation::SqliteObjectMutationSession;
pub(crate) use usage::SqliteUsageWriteSession;

pub struct SqliteStorageBackend {
    pub(in crate::db) db: DbHandle,
    device_id: Arc<Mutex<String>>,
    selective_profile: Arc<Mutex<Option<crate::data_platform::SelectiveSyncProfile>>>,
}

impl SqliteStorageBackend {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self::with_db_handle(DbHandle::from_shared_connection(conn))
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
        drop(guard);
        self.db
            .with_connection(|conn| ensure_usage_sequence_migrated(conn, device_id))
    }

    pub fn device_id(&self) -> String {
        self.device_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
