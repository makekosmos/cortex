/// Bidirectional mapping between CRDT device IDs and Iroh endpoint IDs.
#[derive(Default)]
pub struct DeviceRegistry {
    by_endpoint: Mutex<HashMap<EndpointId, String>>,
    by_device: Mutex<HashMap<String, EndpointId>>,
    trusted: Mutex<HashMap<String, EndpointId>>,
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
        let mut by_endpoint = self.by_endpoint.lock().unwrap_or_else(|e| e.into_inner());
        let mut by_device = self.by_device.lock().unwrap_or_else(|e| e.into_inner());
        let mut trusted = self.trusted.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(previous_device_id) = by_endpoint.insert(endpoint_id, device_id.clone()) {
            by_device.remove(&previous_device_id);
            trusted.remove(&previous_device_id);
        }
        if let Some(previous_endpoint_id) = by_device.insert(device_id.clone(), endpoint_id) {
            by_endpoint.remove(&previous_endpoint_id);
            trusted.remove(&device_id);
        }
    }

    /// Accepts an unauthenticated Hello only when it cannot replace an
    /// already trusted endpoint or device mapping.
    pub fn insert_untrusted(&self, endpoint_id: EndpointId, device_id: String) -> bool {
        let by_endpoint = self.by_endpoint.lock().unwrap_or_else(|e| e.into_inner());
        let by_device = self.by_device.lock().unwrap_or_else(|e| e.into_inner());
        let trusted = self.trusted.lock().unwrap_or_else(|e| e.into_inner());
        if by_endpoint
            .get(&endpoint_id)
            .is_some_and(|current| current != &device_id && trusted.contains_key(current))
            || by_device
                .get(&device_id)
                .is_some_and(|current| *current != endpoint_id && trusted.contains_key(&device_id))
        {
            return false;
        }
        drop(trusted);
        drop(by_device);
        drop(by_endpoint);
        self.insert(endpoint_id, device_id);
        true
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

    pub fn bind_authenticated(&self, device_id: &str, transport_public_key: &str) -> bool {
        let Some(endpoint_id) = self.endpoint_id_for(device_id) else {
            return false;
        };
        if endpoint_id.to_string() != transport_public_key {
            return false;
        }
        self.trusted
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(device_id.to_string(), endpoint_id);
        true
    }

    /// Is this endpoint currently trusted — i.e. may it receive data
    /// frames? A pairing-pending endpoint is mapped but not trusted.
    pub fn is_authenticated_endpoint(&self, endpoint_id: &EndpointId) -> bool {
        self.trusted
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .any(|endpoint| endpoint == endpoint_id)
    }

    /// Drop the trusted binding without forgetting the endpoint→device
    /// mapping — used when a pre-consented Hello fails `handle_message`
    /// checks (HMAC, protocol, removed list) after the transport binding
    /// was already applied.
    pub fn unbind_authenticated(&self, device_id: &str) {
        self.trusted
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(device_id);
    }

    pub fn authenticated_endpoint(&self, device_id: &str) -> Option<EndpointId> {
        self.trusted
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(device_id)
            .copied()
    }

    pub fn remove_endpoint(&self, endpoint_id: &EndpointId) {
        let mut by_endpoint = self.by_endpoint.lock().unwrap_or_else(|e| e.into_inner());
        let mut by_device = self.by_device.lock().unwrap_or_else(|e| e.into_inner());
        let mut trusted = self.trusted.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(device_id) = by_endpoint.remove(endpoint_id) {
            by_device.remove(&device_id);
            trusted.remove(&device_id);
        }
    }
}

// ---------------------------------------------------------------------------
// IrohTransport
// ---------------------------------------------------------------------------
