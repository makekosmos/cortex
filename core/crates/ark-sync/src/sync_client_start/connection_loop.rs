use super::super::{
    AuthenticatedTx, ClientFeedInFlight, ClientOnChangeCallback, ClientOnConnectedCallback,
    ClientOnDisconnectedCallback, ClientOnPeerListCallback, ClientOnRegistryEventsCallback,
    RECONNECT_BASE_MS, RECONNECT_MAX_MS, TAG,
};
use super::connect_and_receive;
use ark_domain::sync_contracts::client_transport::DirectClientConnector;
use ark_domain::sync_storage::SyncRuntimeStorage;
use ark_domain::types::PeerRecord;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

pub(super) struct Context {
    pub(super) storage: Arc<dyn SyncRuntimeStorage>,
    pub(super) peer: Arc<RwLock<PeerRecord>>,
    pub(super) device_id: String,
    pub(super) device_name: String,
    pub(super) space_id: String,
    pub(super) own_addresses: Vec<String>,
    pub(super) auth_secret: Option<String>,
    pub(super) task_merge_bootstrap_v2: bool,
    pub(super) platform: Option<String>,
    pub(super) app_version: Option<String>,
    pub(super) connector: Arc<dyn DirectClientConnector>,
    pub(super) stopped: Arc<AtomicBool>,
    pub(super) authenticated_tx: AuthenticatedTx,
    pub(super) feed_in_flight: ClientFeedInFlight,
    pub(super) on_change: Arc<Mutex<Option<ClientOnChangeCallback>>>,
    pub(super) on_connected: Arc<Mutex<Option<ClientOnConnectedCallback>>>,
    pub(super) on_disconnected: Arc<Mutex<Option<ClientOnDisconnectedCallback>>>,
    pub(super) on_peer_list: Arc<Mutex<Option<ClientOnPeerListCallback>>>,
    pub(super) on_registry_events: Arc<Mutex<Option<ClientOnRegistryEventsCallback>>>,
}

pub(super) async fn run(context: Context) {
    let Context {
        storage,
        peer,
        device_id,
        device_name,
        space_id,
        own_addresses,
        auth_secret,
        task_merge_bootstrap_v2,
        platform,
        app_version,
        connector,
        stopped,
        authenticated_tx,
        feed_in_flight,
        on_change,
        on_connected,
        on_disconnected,
        on_peer_list,
        on_registry_events,
    } = context;
    let mut reconnect_delay = RECONNECT_BASE_MS;

    loop {
        if stopped.load(Ordering::Relaxed) {
            break;
        }

        let peer_record = peer.read().await.clone();
        let mut addresses = peer_record.addresses.clone();

        // Never dial our own listener: a stale record can carry an
        // address that now belongs to us (e.g. the peer's old DHCP
        // lease). Prune it from the stored record — a pure
        // self-reference still ends the client.
        if addresses.iter().any(|a| own_addresses.contains(a)) {
            addresses.retain(|a| !own_addresses.contains(a));
            peer.write().await.addresses = addresses.clone();
            if addresses.is_empty() {
                eprintln!(
                    "{TAG} All addresses for {} are ours — evicting self-record",
                    peer_record.device_name
                );
                stopped.store(true, Ordering::Relaxed);
                break;
            }
        }

        if addresses.is_empty() {
            eprintln!(
                "{TAG} No addresses for peer {}, scheduling reconnect",
                peer_record.device_name
            );
            tokio::time::sleep(Duration::from_millis(reconnect_delay)).await;
            reconnect_delay = (reconnect_delay as f64 * 1.5).min(RECONNECT_MAX_MS as f64) as u64;
            continue;
        }

        let result = connector.connect(&addresses).await;
        let disposition = connect_and_receive::run(
            result,
            connect_and_receive::Context {
                storage: &storage,
                peer: &peer,
                peer_record: &peer_record,
                device_id: &device_id,
                device_name: &device_name,
                space_id: &space_id,
                own_addresses: &own_addresses,
                auth_secret: auth_secret.as_deref(),
                task_merge_bootstrap_v2,
                platform: &platform,
                app_version: &app_version,
                stopped: &stopped,
                authenticated_tx: &authenticated_tx,
                feed_in_flight: &feed_in_flight,
                on_change: &on_change,
                on_connected: &on_connected,
                on_disconnected: &on_disconnected,
                on_peer_list: &on_peer_list,
                on_registry_events: &on_registry_events,
            },
            &mut reconnect_delay,
        )
        .await;

        match disposition {
            connect_and_receive::Disposition::RetryImmediately => continue,
            connect_and_receive::Disposition::Stop => break,
            connect_and_receive::Disposition::Backoff => {}
        }

        let jitter = rand::random::<f64>() * 1000.0;
        let delay = (reconnect_delay as f64 + jitter).min(RECONNECT_MAX_MS as f64);
        tokio::time::sleep(Duration::from_millis(delay as u64)).await;
        reconnect_delay = ((reconnect_delay as f64) * 1.5).min(RECONNECT_MAX_MS as f64) as u64;
    }
}
