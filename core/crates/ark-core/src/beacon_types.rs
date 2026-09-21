// BroadcastDiscovery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BroadcastDiscoveryOptions {
    pub space_id: String,
    pub device_id: String,
    pub device_name: String,
    pub ws_port: u16,
}

pub struct BroadcastDiscovery {
    control_tx: Mutex<Option<mpsc::UnboundedSender<Control>>>,
    on_peer_discovered: Mutex<Option<OnPeerDiscoveredCallback>>,
    seen_peers: Arc<Mutex<HashMap<String, SeenPeer>>>,
}

impl Default for BroadcastDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

