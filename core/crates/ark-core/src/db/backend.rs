use std::sync::{Arc, Mutex};

use super::DbHandle;

mod adapter;
mod core_task;
#[cfg(test)]
mod data_platform_phase7_tests;

pub struct SqliteStorageBackend {
    pub(in crate::db) db: DbHandle,
    device_id: Arc<Mutex<String>>,
    selective_profile: Arc<Mutex<Option<ark_domain::selective_sync::SelectiveSyncProfile>>>,
    #[cfg(test)]
    test_owner: Option<Arc<super::command::DbOwner>>,
}

impl SqliteStorageBackend {
    /// Opens the production writer actor for file-backed integration tests.
    #[cfg(test)]
    pub async fn open_for_test(
        db_path: impl Into<String>,
        legacy_task_device_id: impl Into<String>,
    ) -> Result<Self, String> {
        let (db, owner) = DbHandle::open(db_path.into(), legacy_task_device_id.into())
            .await
            .map_err(|error| error.to_string())?;
        let mut backend = Self::with_db_handle(db);
        backend.test_owner = Some(Arc::new(owner));
        Ok(backend)
    }

    #[cfg(test)]
    pub(crate) async fn load_or_generate_iroh_secret_key_for_test(
        &self,
    ) -> Result<ark_net::iroh::SecretKey, String> {
        crate::iroh_transport::load_or_generate_secret_key_via_actor(&self.db).await
    }

    pub(crate) fn with_db_handle(db: DbHandle) -> Self {
        Self {
            db,
            device_id: Arc::new(Mutex::new(String::new())),
            selective_profile: Arc::new(Mutex::new(None)),
            #[cfg(test)]
            test_owner: None,
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
