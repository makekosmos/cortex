/// Bidirectional mapping between CRDT device IDs and Iroh endpoint IDs.
#[derive(Default)]
pub struct DeviceRegistry {
    by_endpoint: Mutex<HashMap<EndpointId, String>>,
    by_device: Mutex<HashMap<String, EndpointId>>,
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Записывает/перезаписывает маппинг для пары (endpoint_id, device_id).
    /// Перезапись поддержана осознанно: устройство может переподключиться с
    /// тем же `EndpointId`, но потенциально другим заявленным `device_id`
    /// (переименование) — последний `Hello` должен победить.
    pub fn insert(&self, endpoint_id: EndpointId, device_id: String) {
        self.by_endpoint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(endpoint_id, device_id.clone());
        self.by_device
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(device_id, endpoint_id);
    }

    pub fn device_id_for(&self, endpoint_id: &EndpointId) -> Option<String> {
        self.by_endpoint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(endpoint_id)
            .cloned()
    }

    pub fn endpoint_id_for(&self, device_id: &str) -> Option<EndpointId> {
        self.by_device
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(device_id)
            .copied()
    }
}

// ---------------------------------------------------------------------------
// IrohTransport
// ---------------------------------------------------------------------------
