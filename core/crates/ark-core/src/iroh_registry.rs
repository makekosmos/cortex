/// Bidirectional mapping between CRDT device IDs and Iroh endpoint IDs.
#[derive(Default)]
pub struct DeviceRegistry {
    by_endpoint: Mutex<HashMap<EndpointId, String>>,
    by_device: Mutex<HashMap<String, PeerBinding>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PeerBinding {
    endpoint: EndpointId,
    phase: PeerPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeerPhase {
    Observed,
    Bound,
    DataReady,
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
        if let Some(previous_device_id) = by_endpoint.insert(endpoint_id, device_id.clone()) {
            by_device.remove(&previous_device_id);
        }
        if let Some(previous_binding) = by_device.insert(
            device_id.clone(),
            PeerBinding {
                endpoint: endpoint_id,
                phase: PeerPhase::Observed,
            },
        ) {
            let previous_endpoint_id = previous_binding.endpoint;
            if previous_endpoint_id != endpoint_id {
                by_endpoint.remove(&previous_endpoint_id);
            }
        }
    }

    /// Accepts an unauthenticated Hello only when it cannot replace a bound
    /// endpoint or device mapping.
    pub fn insert_untrusted(&self, endpoint_id: EndpointId, device_id: String) -> bool {
        let by_endpoint = self.by_endpoint.lock().unwrap_or_else(|e| e.into_inner());
        let by_device = self.by_device.lock().unwrap_or_else(|e| e.into_inner());
        if by_endpoint.get(&endpoint_id) == Some(&device_id)
            && by_device
                .get(&device_id)
                .is_some_and(|binding| binding.endpoint == endpoint_id)
        {
            return true;
        }
        let is_bound = |id: &str| {
            by_device
                .get(id)
                .is_some_and(|binding| binding.phase != PeerPhase::Observed)
        };
        if by_endpoint
            .get(&endpoint_id)
            .is_some_and(|current| current != &device_id && is_bound(current))
            || by_device
                .get(&device_id)
                .is_some_and(|current| current.endpoint != endpoint_id && is_bound(&device_id))
        {
            return false;
        }
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
            .map(|binding| binding.endpoint)
    }

    pub fn bind_peer_for_control(&self, device_id: &str, transport_public_key: &str) -> bool {
        let Some(endpoint_id) = self.endpoint_id_for(device_id) else {
            return false;
        };
        if endpoint_id.to_string() != transport_public_key {
            return false;
        }
        let mut by_device = self.by_device.lock().unwrap_or_else(|e| e.into_inner());
        let Some(binding) = by_device.get_mut(device_id) else {
            return false;
        };
        if binding.endpoint != endpoint_id {
            return false;
        }
        binding.phase = PeerPhase::Bound;
        true
    }

    pub fn mark_data_ready(&self, device_id: &str, transport_public_key: &str) -> bool {
        let Ok(endpoint_id) = transport_public_key.parse::<EndpointId>() else {
            return false;
        };
        let mut by_device = self.by_device.lock().unwrap_or_else(|e| e.into_inner());
        let Some(binding) = by_device.get_mut(device_id) else {
            return false;
        };
        if binding.endpoint != endpoint_id || binding.phase != PeerPhase::Bound {
            return false;
        }
        binding.phase = PeerPhase::DataReady;
        true
    }

    pub fn is_data_ready_endpoint(&self, endpoint_id: &EndpointId) -> bool {
        self.by_device
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .any(|binding| {
                binding.endpoint == *endpoint_id && binding.phase == PeerPhase::DataReady
            })
    }

    /// Remove admission while retaining the endpoint mapping learned from Hello.
    pub fn unbind_peer(&self, device_id: &str) {
        if let Some(binding) = self
            .by_device
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(device_id)
        {
            binding.phase = PeerPhase::Observed;
        }
    }

    pub fn data_ready_endpoint(&self, device_id: &str) -> Option<EndpointId> {
        self.by_device
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(device_id)
            .filter(|binding| binding.phase == PeerPhase::DataReady)
            .map(|binding| binding.endpoint)
    }

    pub fn remove_endpoint(&self, endpoint_id: &EndpointId) {
        let mut by_endpoint = self.by_endpoint.lock().unwrap_or_else(|e| e.into_inner());
        let mut by_device = self.by_device.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(device_id) = by_endpoint.remove(endpoint_id) {
            by_device.remove(&device_id);
        }
    }
}

// ---------------------------------------------------------------------------
// IrohTransport
// ---------------------------------------------------------------------------
