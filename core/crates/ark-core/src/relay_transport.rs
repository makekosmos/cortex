//! Relay transport: outbound WebSocket client that connects to a relay server
//! and bridges LAN-style `LanSyncMessage` frames over the relay.
//!
//! This is wired through `mesh.rs` and activated when `FfiSyncConfig.relay_url`
//! is `Some(...)`. It is NOT exposed via UniFFI directly.
//!
//! Reconnect strategy: exponential backoff starting at 1 s, doubling each
//! attempt, capped at 60 s.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::protocol::{
    compute_hello_auth_hmac, deserialize_message, generate_auth_nonce, message_origin_device_id,
    normalize_auth_secret, serialize_message, LanSyncMessage, PROTOCOL_VERSION,
};
use crate::sync_transport::{SyncTransport, TransportEvent};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Base WebSocket URL of the relay server, e.g. `wss://relay.example.com`.
    pub url: String,
    pub space_id: String,
    pub device_id: String,
    pub device_name: String,
    pub api_key: String,
    pub auth_secret: Option<String>,
}

// ---------------------------------------------------------------------------
// RelayTransport
// ---------------------------------------------------------------------------

/// Handles connection lifecycle to a relay server.
pub struct RelayTransport {
    config: RelayConfig,
    /// Offline outbox — messages enqueued while the transport is not connected.
    outbox: Arc<Mutex<VecDeque<LanSyncMessage>>>,
    /// Channel sender used by `send()` to push messages into the active loop.
    /// Wrapped in Mutex so we can replace it when `start()` is called.
    send_tx: Mutex<mpsc::UnboundedSender<LanSyncMessage>>,
    /// Shutdown signal sender.
    stop_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl RelayTransport {
    pub fn new(config: RelayConfig) -> Self {
        // Create a placeholder channel whose receiver is immediately dropped;
        // sends will fail and fall back to the outbox until `start()` is called.
        let (send_tx, _) = mpsc::unbounded_channel();
        RelayTransport {
            config,
            outbox: Arc::new(Mutex::new(VecDeque::new())),
            send_tx: Mutex::new(send_tx),
            stop_tx: Mutex::new(None),
        }
    }

    /// Build the relay WebSocket URL with query parameters.
    fn ws_url(&self) -> String {
        format!(
            "{}/ws?space_id={}&device_id={}&api_key={}",
            self.config.url, self.config.space_id, self.config.device_id, self.config.api_key,
        )
    }
}

#[async_trait::async_trait]
impl SyncTransport for RelayTransport {
    /// Start the relay transport. Returns immediately; background tasks drive
    /// the actual connection.  Events are forwarded through `event_tx`.
    async fn start(&self, event_tx: mpsc::UnboundedSender<TransportEvent>) -> Result<(), String> {
        // Create a fresh send channel and a shutdown channel.
        let (send_tx, mut send_rx) = mpsc::unbounded_channel::<LanSyncMessage>();
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();

        // Swap in the new sender so `send()` goes to the active loop.
        *self.send_tx.lock().unwrap_or_else(|e| e.into_inner()) = send_tx.clone();
        *self.stop_tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(stop_tx);

        let outbox = self.outbox.clone();
        let ws_url = self.ws_url();
        let device_id = self.config.device_id.clone();
        let device_name = self.config.device_name.clone();
        let space_id = self.config.space_id.clone();
        let auth_secret = normalize_auth_secret(self.config.auth_secret.clone());

        // Drain any offline outbox into the new channel right away.
        {
            let mut ob = outbox.lock().unwrap_or_else(|e| e.into_inner());
            while let Some(msg) = ob.pop_front() {
                let _ = send_tx.send(msg);
            }
        }

        tokio::spawn(async move {
            let mut backoff_secs: u64 = 1;

            loop {
                let connect_result = connect_async(&ws_url).await;

                match connect_result {
                    Err(e) => {
                        eprintln!("[RelayTransport] connect failed: {e}; retry in {backoff_secs}s");
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(backoff_secs)) => {}
                            _ = &mut stop_rx => { return; }
                        }
                        backoff_secs = (backoff_secs * 2).min(60);
                        continue;
                    }
                    Ok((ws_stream, _)) => {
                        backoff_secs = 1; // reset on successful connect
                        eprintln!("[RelayTransport] connected to relay");

                        let (mut ws_tx, mut ws_rx) = ws_stream.split();

                        // Send hello using the standard LanSyncMessage wire format.
                        let (auth_nonce, auth_hmac) = match auth_secret.as_ref() {
                            Some(secret) => {
                                let nonce = generate_auth_nonce();
                                let hmac =
                                    compute_hello_auth_hmac(secret, &space_id, &device_id, &nonce);
                                (Some(nonce), Some(hmac))
                            }
                            None => (None, None),
                        };
                        let hello = LanSyncMessage::Hello {
                            protocol_version: PROTOCOL_VERSION,
                            device_id: device_id.clone(),
                            device_name: device_name.clone(),
                            space_id: space_id.clone(),
                            addresses: None,
                            auth_nonce,
                            auth_hmac,
                        };
                        let text = serialize_message(&hello);
                        let _ = ws_tx.send(Message::Text(text)).await;

                        // Drain offline outbox on reconnect.
                        {
                            // Collect queued messages without holding the mutex across awaits.
                            let queued: Vec<LanSyncMessage> = {
                                let mut ob = outbox.lock().unwrap_or_else(|e| e.into_inner());
                                ob.drain(..).collect()
                            };
                            for msg in queued {
                                let text = serialize_message(&msg);
                                let _ = ws_tx.send(Message::Text(text)).await;
                            }
                        }

                        let _ = event_tx.send(TransportEvent::Connected {
                            device_id: device_id.clone(),
                        });

                        // Inner send/receive loop.
                        loop {
                            tokio::select! {
                                _ = &mut stop_rx => {
                                    let _ = ws_tx.close().await;
                                    return;
                                }
                                Some(msg_to_send) = send_rx.recv() => {
                                    let text = serialize_message(&msg_to_send);
                                    if ws_tx.send(Message::Text(text)).await.is_err() {
                                        break; // connection dead — fall through to reconnect
                                    }
                                }
                                inbound = ws_rx.next() => {
                                    match inbound {
                                        Some(Ok(Message::Text(text))) => {
                                            if let Some(lan_msg) = deserialize_message(&text) {
                                                let from =
                                                    message_origin_device_id(&lan_msg)
                                                        .unwrap_or_default();
                                                let _ = event_tx.send(TransportEvent::MessageReceived {
                                                    from_device_id: from,
                                                    msg: lan_msg,
                                                });
                                            }
                                        }
                                        Some(Ok(Message::Close(_))) | None => break,
                                        Some(Err(_)) => break,
                                        _ => {}
                                    }
                                }
                            }
                        }

                        let _ = event_tx.send(TransportEvent::Disconnected {
                            device_id: device_id.clone(),
                        });
                    }
                }

                // Reconnect with exponential backoff.
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(backoff_secs)) => {}
                    _ = &mut stop_rx => { return; }
                }
                backoff_secs = (backoff_secs * 2).min(60);
            }
        });

        Ok(())
    }

    /// Enqueue a message. If the transport is connected, it goes immediately;
    /// if disconnected, it is stored in the offline outbox (capped at 500 entries).
    fn send(&self, msg: LanSyncMessage) -> Result<(), String> {
        if matches!(
            &msg,
            LanSyncMessage::SignedIntegrationFrame { .. }
                | LanSyncMessage::SignedIntegrationAck { .. }
        ) {
            return Err("relay broadcast cannot carry addressed integration messages".into());
        }
        let tx = self.send_tx.lock().unwrap_or_else(|e| e.into_inner());
        if tx.send(msg.clone()).is_err() {
            // Channel closed (not yet started or stopped) — use offline outbox.
            const MAX_OUTBOX_SIZE: usize = 500;
            let mut outbox = self.outbox.lock().unwrap_or_else(|e| e.into_inner());
            if outbox.len() < MAX_OUTBOX_SIZE {
                outbox.push_back(msg);
            }
        }
        Ok(())
    }

    /// Signal the background loop to stop.
    fn stop(&self) {
        if let Some(tx) = self
            .stop_tx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = tx.send(());
        }
    }
}
