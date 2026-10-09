/// Params of `start_sync`. The variant is a newtype over this struct so the
/// handler receives the request params object directly.
#[derive(Debug, Deserialize)]
pub(crate) struct StartSyncParams {
    pub(crate) space_id: String,
    pub(crate) device_id: String,
    #[serde(default)]
    pub(crate) device_name: Option<String>,
    #[serde(default)]
    pub(crate) port: Option<u16>,
    #[serde(default)]
    pub(crate) seed_addresses: Option<Vec<String>>,
    /// Optional relay server WebSocket URL (e.g. "wss://relay.example.com").
    #[serde(default)]
    pub(crate) relay_url: Option<String>,
    /// API key for the relay server.
    #[serde(default)]
    pub(crate) relay_api_key: Option<String>,
    /// Optional shared secret for LAN/P2P hello HMAC authentication.
    #[serde(default)]
    pub(crate) auth_secret: Option<String>,
    /// Step 4a: select the iroh p2p transport instead of relay — see
    /// `select_transport`.
    #[serde(default)]
    pub(crate) use_iroh: bool,
    /// Pairing ticket string for the iroh peer (see
    /// `iroh_transport::IrohTransport::our_ticket`/`from_ticket`).
    #[serde(default)]
    pub(crate) iroh_peer_ticket: Option<String>,
    /// KOS-369: set only when this start IS the user entering a code
    /// (`connect_with_pairing_code`) — marks the endpoint as a
    /// human-consented outgoing pairing and surfaces it in the snapshot.
    /// Boot restores replay the stored ticket without it.
    #[serde(default)]
    pub(crate) pairing_connect: bool,
    /// Whether to start LAN beacon discovery. Defaults to true for compatibility.
    #[serde(default = "default_discovery_enabled")]
    pub(crate) discovery_enabled: bool,
    /// Where the sync stack binds its listeners. `loopback` binds every
    /// socket to 127.0.0.1 / ::1 (tests, single-machine pairing);
    /// `all_interfaces` (default) keeps LAN behaviour.
    #[serde(default)]
    pub(crate) bind: SyncBind,
    /// Product version label the embedder advertises in Hello frames
    /// (shown to peers in their device list). `None` omits the field.
    #[serde(default)]
    pub(crate) app_version: Option<String>,
}
