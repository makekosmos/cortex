use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct EnrollPairedDeviceCommand {
    local_device_id: String,
    target_device_id: String,
    transport_public_key: String,
}

impl EnrollPairedDeviceCommand {
    pub(crate) fn new(
        local_device_id: String,
        target_device_id: String,
        transport_public_key: String,
    ) -> Self {
        Self {
            local_device_id,
            target_device_id,
            transport_public_key,
        }
    }

    pub(crate) fn into_parts(self) -> (String, String, String) {
        (
            self.local_device_id,
            self.target_device_id,
            self.transport_public_key,
        )
    }
}
