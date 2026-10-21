use std::{collections::HashMap, path::Path};

use ark_domain::device_registry::RegistryEvent;

use crate::device_registry::{DeviceRegistryEntry, DeviceRegistryIdentity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRegistryState {
    pub entries: Vec<DeviceRegistryEntry>,
    pub events: Vec<RegistryEvent>,
    pub identities: HashMap<String, DeviceRegistryIdentity>,
    pub transport_key_pins: HashMap<String, String>,
}

/// Owns registry fault injection and persisted-state inspection for Core tests.
pub struct DeviceRegistryFixture {
    connection: rusqlite::Connection,
}

impl DeviceRegistryFixture {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "device registry database path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        Ok(Self { connection })
    }

    pub fn reject_device_registry_projection(&self) -> Result<(), String> {
        crate::test_support::reject_device_registry_projection(&self.connection)
            .map_err(|error| error.to_string())
    }

    pub fn reject_remote_device_rename(&self) -> Result<(), String> {
        crate::test_support::reject_remote_device_rename(&self.connection)
            .map_err(|error| error.to_string())
    }

    pub fn has_inbox_receipt(&self, message_id: &str) -> Result<bool, String> {
        crate::test_support::has_inbox_receipt(&self.connection, message_id)
            .map_err(|error| error.to_string())
    }

    pub fn object_exists(&self, object_id: &str) -> Result<bool, String> {
        crate::objects::get_object(&self.connection, object_id).map(|object| object.is_some())
    }

    pub fn inspect(&self) -> Result<DeviceRegistryState, String> {
        let (_, _, events) = crate::device_registry::load_events_page(
            &self.connection,
            0,
            None,
            crate::device_registry::MAX_REGISTRY_EVENTS_PAGE_SIZE,
        )?;
        let entries = crate::device_registry::load_entries(&self.connection)?;
        let mut identities = HashMap::new();
        for entry in &entries {
            if let Some(identity) =
                crate::device_registry::load_identity(&self.connection, &entry.device_id)?
            {
                identities.insert(entry.device_id.clone(), identity);
            }
        }
        let transport_key_pins = crate::sync_kv::get_sync_kv(
            &self.connection,
            crate::device_registry::PEER_TRANSPORT_KEYS_KEY,
        )?
        .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
        .transpose()?
        .unwrap_or_default();

        Ok(DeviceRegistryState {
            entries,
            events,
            identities,
            transport_key_pins,
        })
    }
}
