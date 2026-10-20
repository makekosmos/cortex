use super::DeviceRegistryReadCapabilities;
use ark_domain::device_registry::RegistryEvent;

impl DeviceRegistryReadCapabilities<'_> {
    pub fn local_device_id(&self) -> Result<Option<String>, String> {
        crate::sync_kv::get_sync_kv(
            self.connection,
            crate::sync_kv::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY,
        )
    }

    pub fn entries(&self) -> Result<Vec<crate::device_registry::DeviceRegistryEntry>, String> {
        crate::device_registry::load_entries(self.connection)
    }

    pub fn events_page(
        &self,
        after_position: i64,
        high_watermark: Option<i64>,
        limit: usize,
    ) -> Result<(i64, usize, Vec<RegistryEvent>), String> {
        crate::device_registry::load_events_page(
            self.connection,
            after_position,
            high_watermark,
            limit,
        )
    }
}
