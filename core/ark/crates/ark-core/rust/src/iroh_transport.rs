//! Iroh p2p QUIC transport — Фаза 0 spike (GREEN stage — bidirectional).
//!
//! Зеркалит публичную форму `relay_transport.rs` (`RelayConfig`/`RelayEvent`/
//! `RelayTransport`), чтобы фаза 1 могла относительно механически встроить
//! этот транспорт в `mesh.rs`. См. spec:
//! `.agent/tasks/2026-06-16-iroh-transport/spec.md`.
//!
//! ## Дизайн (GREEN bidirectional revision)
//!
//! Каждое QUIC-соединение (входящее или исходящее) живёт долго: один bi-стрим
//! на всё время соединения, кадры идут back-to-back без finish() между ними.
//! Это зеркалит `RelayTransport`: один WebSocket-стрим на соединение.
//!
//! `send()` не диалит новый conn на каждый вызов — он кладёт сообщение в
//! `broadcast::Sender`, на который подписывается каждая активная connection-
//! задача. Если подписчиков нет (нет соединения) — сообщение молча дропается:
//! пропущенные pre-connect live-changes будут скомпенсированы VersionVector
//! обменом при (ре)коннекте.
//!
//! При подключении транспорт автоматически инжектирует аутентифицированный
//! `Hello` как первый фрейм (аналогично `relay_transport.rs` строки 128-148).
//! Это запускает CRDT-рукопожатие через `RelaySync::handle_message` без
//! явного вызова `send(Hello)` сверху.
//!
//! Dial-loop (если `config.peer_addr/peer_ticket`) пересоединяется с
//! экспоненциальным backoff (1s → 2s → ... → 60s) при обрыве — зеркалит
//! RelayTransport.
//!
//! Shutdown: `tokio::sync::watch::channel(false)` позволяет нескольким задачам
//! одновременно наблюдать сигнал остановки (в отличие от oneshot).
//!
//! Relay mode: `config.relay_mode` управляет NAT traversal. Production
//! call sites (`main.rs::handle_start_sync`, `ffi.rs`) оставляют его
//! `None`, что резолвится в `RelayMode::Default` (n0 production relay
//! servers) — без этого два узла за разными NAT/firewall не могут
//! соединиться. Тест `iroh_round_trip.rs` явно передаёт
//! `Some(RelayMode::Disabled)`, чтобы остаться offline в CI.
//!
//! Доступен только под feature `iroh-spike` — production build (`cargo build`
//! без флагов) не подтягивает крейт `iroh` и не компилирует этот файл.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Mutex;
use std::time::Duration;

use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode};
use iroh_tickets::endpoint::EndpointTicket;
use iroh_tickets::Ticket;
use rusqlite::Connection;
use tokio::sync::{broadcast, mpsc, watch};

use crate::db::{get_sync_kv, set_sync_kv};
use crate::protocol::{
    compute_hello_auth_hmac, deserialize_message, generate_auth_nonce, message_origin_device_id,
    normalize_auth_secret, serialize_message, LanSyncMessage, PROTOCOL_VERSION,
};
use crate::sync_transport::{SyncTransport, TransportEvent};

/// `sync_kv` ключ для персистентного iroh identity secret key (см. spec
/// фазы 1 шаг 1). Хранится hex-кодированием 32 байт `SecretKey::to_bytes()`
/// — тот же паттерн, что у `lan_sync.version_vector` в `db.rs`: одна строка
/// в существующей key-value таблице, без миграции схемы.
const IROH_SECRET_KEY_KV_KEY: &str = "iroh.secret_key";

/// ALPN-идентификатор протокола ark-sync поверх iroh QUIC-стримов.
/// Версионируем отдельно от `protocol::PROTOCOL_VERSION` (см. spec §3.3).
pub const ARK_SYNC_ALPN: &[u8] = b"ark-sync/1";

/// Ёмкость broadcast-канала исходящих сообщений. Выбрана щедро: реальные
/// CRDT-потоки не достигают такого burst. При `RecvError::Lagged` задача
/// просто пропускает старые кадры — они будут компенсированы VV-обменом
/// при следующем (ре)коннекте (см. модульный doc).
const OUTGOING_BROADCAST_CAPACITY: usize = 256;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Конфигурация iroh-транспорта для одного узла (peer фазы 0 задаётся явно,
/// без discovery/pairing UI — см. spec §2/§3.2).
#[derive(Clone)]
pub struct IrohConfig {
    /// Наш текущий идентификатор устройства (как в LAN/relay-транспортах).
    pub device_id: String,
    pub device_name: String,
    /// Пространство синхронизации (прокидывается как есть, не используется
    /// на транспортном уровне iroh — только для согласованности с другими
    /// транспортами).
    pub space_id: String,
    /// Секретный ключ identity. `None` => генерируется in-memory при `start()`
    /// (не персистится — см. spec §2, открытый вопрос §4.3).
    pub secret_key: Option<iroh::SecretKey>,
    /// Адрес пира, с которым нужно установить прямое соединение. В фазе 0
    /// передаётся явно тестом/вызывающим кодом, без discovery.
    pub peer_addr: Option<iroh::EndpointAddr>,
    /// Шаг 3: альтернатива `peer_addr` — единая ticket-строка (см.
    /// `our_ticket()`/`from_ticket()`), из которой `EndpointAddr` пира
    /// извлекается лениво при первом подключении. Если задан `peer_addr`,
    /// он имеет приоритет (явный адрес важнее тикета).
    pub peer_ticket: Option<String>,
    /// Dev-flow cross-network step: relay mode override for `start()`.
    /// `None` => production default — `RelayMode::Default` (n0 relay
    /// servers), needed for real NAT traversal between two machines.
    /// `Some(RelayMode::Disabled)` is for tests only (`iroh_round_trip.rs`):
    /// offline loopback on one machine must not depend on network/relay
    /// availability in CI. Production call sites (`main.rs::handle_start_sync`,
    /// `ffi.rs`) leave this `None`.
    pub relay_mode: Option<RelayMode>,
    /// Опциональный секрет для HMAC-аутентификации Hello (аналог
    /// `RelayConfig::auth_secret`). `None` => Hello отправляется без HMAC.
    pub auth_secret: Option<String>,
}

// ---------------------------------------------------------------------------
// Ticket-based pairing — Шаг 3.
// ---------------------------------------------------------------------------

/// Строит ticket-строку из `EndpointAddr` — обратная сторона `from_ticket`.
/// Свободная функция (а не метод `IrohTransport`), чтобы unit-тест мог
/// проверить round-trip без поднятия реального `Endpoint`.
fn ticket_string(addr: &EndpointAddr) -> String {
    EndpointTicket::new(addr.clone()).encode_string()
}

/// Парсит ticket-строку (см. `IrohTransport::our_ticket`) обратно в
/// `EndpointAddr`, который можно использовать для `endpoint.connect(...)`.
///
/// Тип тикета — `iroh_tickets::endpoint::EndpointTicket` (крейт
/// `iroh-tickets` 1.0.0; `iroh` 1.0.0 сам его не реэкспортирует — проверено
/// по исходникам `iroh-1.0.0/src/lib.rs`, там нет `Ticket`/`ticket`).
/// Канонический строковый вид — префикс `"endpoint"` + base32 без паддинга
/// (см. `Ticket::encode_string`/`decode_string` в `iroh-tickets`).
pub fn from_ticket(ticket: &str) -> Result<EndpointAddr, String> {
    EndpointTicket::decode_string(ticket.trim())
        .map(|t| t.endpoint_addr().clone())
        .map_err(|e| format!("iroh transport: invalid ticket: {e}"))
}

// ---------------------------------------------------------------------------
// device_id ↔ EndpointId реестр — Шаг 3.
// ---------------------------------------------------------------------------

/// In-memory соответствие CRDT `device_id` (как в `LanSyncMessage::Hello`) и
/// транспортного `EndpointId` пира. Заполняется на принимающей стороне при
/// первом входящем `Hello` от соединения — до этого момента `EndpointId`
/// пира известен, но какой это CRDT-девайс — нет.
///
/// Хранит обе проекции (`EndpointId -> device_id` и `device_id ->
/// EndpointId`), а не одну с линейным поиском — обе стороны нужны: accept-
/// loop резолвит `from_device_id` по `EndpointId` соединения,
/// потенциальный будущий outbound-routing — наоборот.
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

/// Handles connection lifecycle for the iroh p2p transport.
pub struct IrohTransport {
    config: IrohConfig,
    /// Запущенный `Endpoint` после `start()`. `None` до старта.
    endpoint: Mutex<Option<Endpoint>>,
    /// Broadcast-канал исходящих сообщений — `send()` кладёт сюда, каждая
    /// активная connection-задача подписывается и пишет в свой QUIC bi-стрим.
    out_tx: broadcast::Sender<LanSyncMessage>,
    /// Shutdown watch channel. `true` = нужно остановиться.
    stop_tx: watch::Sender<bool>,
    stop_rx: watch::Receiver<bool>,
    /// Шаг 3: device_id ↔ EndpointId реестр, заполняется по входящим Hello.
    registry: std::sync::Arc<DeviceRegistry>,
}

impl IrohTransport {
    pub fn new(config: IrohConfig) -> Self {
        let (out_tx, _) = broadcast::channel(OUTGOING_BROADCAST_CAPACITY);
        let (stop_tx, stop_rx) = watch::channel(false);
        IrohTransport {
            config,
            endpoint: Mutex::new(None),
            out_tx,
            stop_tx,
            stop_rx,
            registry: std::sync::Arc::new(DeviceRegistry::new()),
        }
    }

    /// Текущий снимок реестра device_id ↔ EndpointId (для тестов/диагностики).
    #[doc(hidden)]
    pub fn registry(&self) -> &DeviceRegistry {
        &self.registry
    }

    /// Тестовый хук: то же кодирование, что использует `our_ticket()`, но
    /// без необходимости поднимать реальный `Endpoint`.
    #[doc(hidden)]
    pub fn ticket_string_for_addr(addr: &EndpointAddr) -> String {
        ticket_string(addr)
    }

    /// Возвращает нашу ticket-строку — то, что нужно передать другому
    /// устройству для pairing. Требует, чтобы транспорт уже был запущен.
    pub async fn our_ticket(&self) -> Result<String, String> {
        let relay_disabled = matches!(self.config.relay_mode, Some(RelayMode::Disabled));
        if relay_disabled {
            let addr = self.endpoint_addr().ok_or_else(|| {
                "iroh transport: our_ticket() called before start()".to_string()
            })?;
            return Ok(ticket_string(&addr));
        }

        let endpoint = self
            .endpoint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| "iroh transport: our_ticket() called before start()".to_string())?;
        endpoint.online().await;
        Ok(ticket_string(&endpoint.addr()))
    }

    /// Резолвит эффективный адрес пира: явный `peer_addr` имеет приоритет,
    /// иначе парсим `peer_ticket`.
    fn resolve_peer_addr(&self) -> Result<EndpointAddr, String> {
        if let Some(addr) = self.config.peer_addr.clone() {
            return Ok(addr);
        }
        let ticket = self.config.peer_ticket.as_deref().ok_or_else(|| {
            "iroh transport: send() requires config.peer_addr or config.peer_ticket".to_string()
        })?;
        from_ticket(ticket)
    }

    /// Возвращает локальный `EndpointId` транспорта, если он уже запущен.
    pub fn endpoint_id(&self) -> Option<EndpointId> {
        self.endpoint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|ep| ep.id())
    }

    /// Возвращает локальный адрес транспорта (для ручной передачи пиру в
    /// фазе 0, без pairing UI/discovery). Использует `bound_sockets()` +
    /// явную подстановку `127.0.0.1`.
    pub fn endpoint_addr(&self) -> Option<EndpointAddr> {
        let guard = self.endpoint.lock().unwrap_or_else(|e| e.into_inner());
        let ep = guard.as_ref()?;
        let port = ep
            .bound_sockets()
            .iter()
            .find(|addr| addr.is_ipv4())
            .map(|addr| addr.port())?;
        let socket: SocketAddr = (Ipv4Addr::LOCALHOST, port).into();
        Some(EndpointAddr::new(ep.id()).with_ip_addr(socket))
    }

    /// Строит аутентифицированный `Hello` для инжекции при подключении.
    /// Зеркалит `relay_transport.rs` строки 128-148.
    fn make_hello(&self) -> LanSyncMessage {
        let auth_secret = normalize_auth_secret(self.config.auth_secret.clone());
        let (auth_nonce, auth_hmac) = match auth_secret.as_ref() {
            Some(secret) => {
                let nonce = generate_auth_nonce();
                let hmac = compute_hello_auth_hmac(
                    secret,
                    &self.config.space_id,
                    &self.config.device_id,
                    &nonce,
                );
                (Some(nonce), Some(hmac))
            }
            None => (None, None),
        };
        LanSyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: self.config.device_id.clone(),
            device_name: self.config.device_name.clone(),
            space_id: self.config.space_id.clone(),
            addresses: None,
            auth_nonce,
            auth_hmac,
        }
    }
}

/// Общий обработчик одного долгоживущего QUIC-соединения (inbound или
/// outbound). Вызывается из accept-loop (is_dialer=false) и dial-loop
/// (is_dialer=true).
///
/// Логика:
/// 1. Получает bi-стрим (dialer открывает, listener принимает).
/// 2. Инжектирует аутентифицированный Hello как первый исходящий фрейм.
/// 3. Эмитит `TransportEvent::Connected`.
/// 4. Запускает select!-loop: параллельно пишет из broadcast + читает входящие.
/// 5. При выходе из loop'а эмитит `TransportEvent::Disconnected`.
async fn handle_connection(
    conn: iroh::endpoint::Connection,
    is_dialer: bool,
    hello: LanSyncMessage,
    mut out_rx: broadcast::Receiver<LanSyncMessage>,
    mut stop_rx: watch::Receiver<bool>,
    event_tx: mpsc::UnboundedSender<TransportEvent>,
    registry: std::sync::Arc<DeviceRegistry>,
) {
    let remote_endpoint_id = conn.remote_id();

    // Получаем единственный bi-стрим для всего соединения.
    let (mut send, mut recv) = if is_dialer {
        match conn.open_bi().await {
            Ok(streams) => streams,
            Err(e) => {
                eprintln!("[IrohTransport] open_bi failed: {e}");
                return;
            }
        }
    } else {
        match conn.accept_bi().await {
            Ok(streams) => streams,
            Err(e) => {
                eprintln!("[IrohTransport] accept_bi failed: {e}");
                return;
            }
        }
    };

    // Инжектируем Hello как первый фрейм — запускает CRDT-рукопожатие
    // через RelaySync::handle_message без явного send() сверху.
    let hello_payload = serialize_message(&hello);
    if let Err(e) = write_frame(&mut send, hello_payload.as_bytes()).await {
        eprintln!("[IrohTransport] write Hello frame failed: {e}");
        return;
    }

    // Резолвим наш device_id для Connected/Disconnected событий.
    let our_device_id = match &hello {
        LanSyncMessage::Hello { device_id, .. } => device_id.clone(),
        _ => String::new(),
    };

    let _ = event_tx.send(TransportEvent::Connected {
        device_id: our_device_id.clone(),
    });

    // Select!-loop: параллельно пишем исходящие + читаем входящие.
    loop {
        tokio::select! {
            // Shutdown signal.
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    let _ = send.finish();
                    break;
                }
            }

            // Исходящее сообщение из broadcast-канала.
            recv_result = out_rx.recv() => {
                match recv_result {
                    Ok(msg) => {
                        let payload = serialize_message(&msg);
                        if let Err(e) = write_frame(&mut send, payload.as_bytes()).await {
                            eprintln!("[IrohTransport] write_frame failed: {e}");
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Burst превысил ёмкость канала — пропускаем старые кадры;
                        // они будут скомпенсированы VV-обменом при следующем
                        // коннекте. Продолжаем работу без прерывания.
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        // Sender дропнут — транспорт остановлен.
                        break;
                    }
                }
            }

            // Входящий фрейм от пира.
            frame_result = read_frame(&mut recv) => {
                match frame_result {
                    Ok(raw) => {
                        if let Some(msg) = deserialize_message(&raw) {
                            // На Hello учим реестр (EndpointId → CRDT device_id).
                            if let LanSyncMessage::Hello { device_id, .. } = &msg {
                                registry.insert(remote_endpoint_id, device_id.clone());
                            }
                            // Резолвим from_device_id: сначала из самого сообщения
                            // (Hello/VersionVector/LiveChange с origin_device_id),
                            // иначе — из реестра по EndpointId соединения.
                            let from_device_id = message_origin_device_id(&msg)
                                .or_else(|| registry.device_id_for(&remote_endpoint_id))
                                .unwrap_or_default();
                            let _ = event_tx.send(TransportEvent::MessageReceived {
                                from_device_id,
                                msg,
                            });
                        }
                    }
                    Err(e) => {
                        // Стрим закрыт или сброшен — выходим, эмитим Disconnected.
                        eprintln!("[IrohTransport] read_frame ended: {e}");
                        break;
                    }
                }
            }
        }
    }

    // Резолвируем финальный device_id для Disconnected из реестра (может
    // быть заполнен, если Hello уже пришёл).
    let disconnected_device_id = registry
        .device_id_for(&remote_endpoint_id)
        .unwrap_or(our_device_id);
    let _ = event_tx.send(TransportEvent::Disconnected {
        device_id: disconnected_device_id,
    });
}

#[async_trait::async_trait]
impl SyncTransport for IrohTransport {
    /// Запускает транспорт: байндит `Endpoint`, поднимает accept-loop для
    /// входящих соединений и (если `config.peer_addr/peer_ticket`) dial-loop
    /// с экспоненциальным backoff.
    async fn start(&self, event_tx: mpsc::UnboundedSender<TransportEvent>) -> Result<(), String> {
        let relay_mode = self.config.relay_mode.clone().unwrap_or(RelayMode::Default);

        let mut builder = Endpoint::builder(presets::Minimal)
            .relay_mode(relay_mode)
            .alpns(vec![ARK_SYNC_ALPN.to_vec()]);

        if let Some(secret_key) = self.config.secret_key.clone() {
            builder = builder.secret_key(secret_key);
        }

        let endpoint = builder
            .bind()
            .await
            .map_err(|e| format!("iroh transport: endpoint bind failed: {e}"))?;

        *self.endpoint.lock().unwrap_or_else(|e| e.into_inner()) = Some(endpoint.clone());

        // ── Accept-loop ───────────────────────────────────────────────────────
        // Каждое входящее соединение порождает `handle_connection(is_dialer=false)`.
        {
            let accept_endpoint = endpoint.clone();
            let accept_event_tx = event_tx.clone();
            let accept_registry = self.registry.clone();
            let mut accept_stop = self.stop_rx.clone();
            let out_tx = self.out_tx.clone();
            let hello = self.make_hello();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = accept_stop.changed() => {
                            if *accept_stop.borrow() { return; }
                        }
                        incoming = accept_endpoint.accept() => {
                            let Some(incoming) = incoming else {
                                // Endpoint закрыт.
                                return;
                            };
                            let event_tx = accept_event_tx.clone();
                            let registry = accept_registry.clone();
                            let out_rx = out_tx.subscribe();
                            let stop_rx = accept_stop.clone();
                            let hello = hello.clone();

                            tokio::spawn(async move {
                                let conn = match incoming.await {
                                    Ok(conn) => conn,
                                    Err(e) => {
                                        eprintln!("[IrohTransport] incoming connection failed: {e}");
                                        return;
                                    }
                                };
                                handle_connection(
                                    conn,
                                    false, // is_dialer
                                    hello,
                                    out_rx,
                                    stop_rx,
                                    event_tx,
                                    registry,
                                )
                                .await;
                            });
                        }
                    }
                }
            });
        }

        // ── Dial-loop (если задан peer) ───────────────────────────────────────
        // Подключается к пиру, запускает `handle_connection(is_dialer=true)`,
        // при разрыве переподключается с экспоненциальным backoff.
        if self.config.peer_addr.is_some() || self.config.peer_ticket.is_some() {
            let peer_addr = self.resolve_peer_addr()?;
            let dial_endpoint = endpoint.clone();
            let dial_event_tx = event_tx.clone();
            let dial_registry = self.registry.clone();
            let mut dial_stop = self.stop_rx.clone();
            let out_tx = self.out_tx.clone();
            let hello = self.make_hello();

            tokio::spawn(async move {
                let mut backoff_secs: u64 = 1;

                loop {
                    // Проверяем stop до попытки коннекта.
                    if *dial_stop.borrow() {
                        return;
                    }

                    match dial_endpoint
                        .connect(peer_addr.clone(), ARK_SYNC_ALPN)
                        .await
                    {
                        Err(e) => {
                            eprintln!(
                                "[IrohTransport] connect to peer failed: {e}; retry in {backoff_secs}s"
                            );
                            tokio::select! {
                                _ = tokio::time::sleep(Duration::from_secs(backoff_secs)) => {}
                                _ = dial_stop.changed() => {
                                    if *dial_stop.borrow() { return; }
                                }
                            }
                            backoff_secs = (backoff_secs * 2).min(60);
                        }
                        Ok(conn) => {
                            backoff_secs = 1; // сбрасываем backoff при успехе
                            let out_rx = out_tx.subscribe();
                            let stop_rx = dial_stop.clone();
                            handle_connection(
                                conn,
                                true, // is_dialer
                                hello.clone(),
                                out_rx,
                                stop_rx,
                                dial_event_tx.clone(),
                                dial_registry.clone(),
                            )
                            .await;

                            // handle_connection вернулась — соединение закрыто.
                            // Переподключаемся с backoff (если не остановлены).
                            if *dial_stop.borrow() {
                                return;
                            }
                            tokio::select! {
                                _ = tokio::time::sleep(Duration::from_secs(backoff_secs)) => {}
                                _ = dial_stop.changed() => {
                                    if *dial_stop.borrow() { return; }
                                }
                            }
                            backoff_secs = (backoff_secs * 2).min(60);
                        }
                    }
                }
            });
        }

        Ok(())
    }

    /// Ставит сообщение в broadcast-канал исходящих. Все активные соединения
    /// получат его через свои `out_rx`. Если подписчиков нет (соединение ещё
    /// не установлено), сообщение молча дропается: pre-connect live-changes
    /// будут скомпенсированы VersionVector обменом при (ре)коннекте.
    fn send(&self, msg: LanSyncMessage) -> Result<(), String> {
        // send() возвращает Err только если нет подписчиков — это нормально
        // (нет активного соединения). Не возвращаем ошибку вызывающему.
        let _ = self.out_tx.send(msg);
        Ok(())
    }

    /// Сигнализирует всем фоновым задачам остановиться и закрывает `Endpoint`.
    fn stop(&self) {
        let _ = self.stop_tx.send(true);
        // Закрываем endpoint явно — это разбудит accept_endpoint.accept().
        if let Some(ep) = self
            .endpoint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            // close() — async fn, но мы на sync пути. Spawn best-effort close.
            tokio::spawn(async move {
                ep.close().await;
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Framing: 4-байтовый big-endian length prefix + JSON
// ---------------------------------------------------------------------------

/// Верхняя граница для длины фрейма на чтении — без неё `len` из 4-байтового
/// префикса (приходит от пира, untrusted) напрямую идёт в `vec![0u8; len]`,
/// то есть hostile/corrupted префикс (`u32::MAX`) выделяет до ~4 GiB за один
/// `read_frame`. Реальные CRDT JSON-сообщения (`LanSyncMessage`) на порядки
/// меньше; 16 MiB — щедрый запас, не специфичная для протокола константа.
const MAX_FRAME_LEN: usize = 16 * 1024 * 1024;

async fn write_frame(
    send: &mut iroh::endpoint::SendStream,
    payload: &[u8],
) -> Result<(), String> {
    let len = u32::try_from(payload.len())
        .map_err(|_| "iroh transport: frame payload too large".to_string())?;
    send.write_all(&len.to_be_bytes())
        .await
        .map_err(|e| format!("iroh transport: write frame length failed: {e}"))?;
    send.write_all(payload)
        .await
        .map_err(|e| format!("iroh transport: write frame payload failed: {e}"))?;
    Ok(())
}

async fn read_frame(recv: &mut iroh::endpoint::RecvStream) -> Result<String, String> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| format!("iroh transport: read frame length failed: {e}"))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        return Err(format!(
            "iroh transport: frame length {len} exceeds max {MAX_FRAME_LEN}"
        ));
    }

    let mut payload = vec![0u8; len];
    recv.read_exact(&mut payload)
        .await
        .map_err(|e| format!("iroh transport: read frame payload failed: {e}"))?;

    String::from_utf8(payload).map_err(|e| format!("iroh transport: frame payload not utf8: {e}"))
}

// ---------------------------------------------------------------------------
// Identity — Фаза 1 шаг 1: постоянный iroh `SecretKey` на устройство.
// ---------------------------------------------------------------------------

/// Загружает persisted iroh `SecretKey` из `sync_kv` (ключ
/// `iroh.secret_key`) либо генерирует новый и сохраняет его туда же.
///
/// Это даёт стабильный `EndpointId` между рестартами процесса.
///
/// Принимает `&Connection` напрямую (как `get_sync_kv`/`set_sync_kv` в
/// `db.rs`), а не `Arc<dyn StorageBackend>`.
pub fn load_or_generate_secret_key(conn: &Connection) -> Result<iroh::SecretKey, String> {
    if let Some(stored) = get_sync_kv(conn, IROH_SECRET_KEY_KV_KEY)? {
        let bytes = hex_decode_32(&stored)
            .ok_or_else(|| "iroh transport: stored secret key is not valid hex".to_string())?;
        return Ok(iroh::SecretKey::from_bytes(&bytes));
    }

    let secret_key = iroh::SecretKey::generate();
    let encoded = hex_encode(&secret_key.to_bytes());
    set_sync_kv(conn, IROH_SECRET_KEY_KV_KEY, &encoded)?;
    Ok(secret_key)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode_32(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes = s.as_bytes();
    for i in 0..32 {
        let hi = (bytes[i * 2] as char).to_digit(16)?;
        let lo = (bytes[i * 2 + 1] as char).to_digit(16)?;
        out[i] = ((hi << 4) | lo) as u8;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Tests — unit-тесты (ticket codec, registry, identity).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod ticket_tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use iroh::{EndpointAddr, SecretKey};

    use super::{from_ticket, IrohTransport};

    fn sample_addr() -> EndpointAddr {
        let secret = SecretKey::generate();
        let socket: SocketAddr = (Ipv4Addr::LOCALHOST, 4242).into();
        EndpointAddr::new(secret.public()).with_ip_addr(socket)
    }

    #[test]
    fn ticket_round_trips_through_string() {
        let addr = sample_addr();
        let expected_id = addr.id;

        let ticket_str = IrohTransport::ticket_string_for_addr(&addr);
        let parsed = from_ticket(&ticket_str).expect("ticket should parse back");

        assert_eq!(
            parsed.id, expected_id,
            "round-tripped ticket should preserve EndpointId"
        );
    }

    #[test]
    fn from_ticket_rejects_garbage_string() {
        let result = from_ticket("not-a-real-ticket");
        assert!(result.is_err(), "garbage string must not parse as a ticket");
    }
}

#[cfg(test)]
mod registry_tests {
    use iroh::SecretKey;

    use super::DeviceRegistry;

    #[test]
    fn resolves_device_id_by_endpoint_id_after_insert() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        registry.insert(endpoint_id, "device-A".to_string());

        assert_eq!(
            registry.device_id_for(&endpoint_id),
            Some("device-A".to_string())
        );
    }

    #[test]
    fn resolves_endpoint_id_by_device_id_after_insert() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        registry.insert(endpoint_id, "device-A".to_string());

        assert_eq!(
            registry.endpoint_id_for("device-A"),
            Some(endpoint_id)
        );
    }

    #[test]
    fn unknown_endpoint_id_resolves_to_none() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        assert_eq!(registry.device_id_for(&endpoint_id), None);
    }

    #[test]
    fn later_insert_overwrites_earlier_mapping_for_same_endpoint_id() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        registry.insert(endpoint_id, "device-A".to_string());
        registry.insert(endpoint_id, "device-A-renamed".to_string());

        assert_eq!(
            registry.device_id_for(&endpoint_id),
            Some("device-A-renamed".to_string())
        );
    }
}

#[cfg(test)]
mod identity_tests {
    use rusqlite::Connection;

    use crate::db::init_schema;

    use super::load_or_generate_secret_key;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn secret_key_is_persisted_across_calls_on_same_storage() {
        let conn = setup_db();

        let key_a = load_or_generate_secret_key(&conn).expect("first load/generate");
        let key_b = load_or_generate_secret_key(&conn).expect("second load/generate");

        assert_eq!(
            key_a.public(),
            key_b.public(),
            "EndpointId должен быть стабилен между вызовами на одном storage"
        );
    }

    #[test]
    fn secret_key_differs_across_independent_storages() {
        let conn_1 = setup_db();
        let conn_2 = setup_db();

        let key_1 = load_or_generate_secret_key(&conn_1).expect("storage 1 load/generate");
        let key_2 = load_or_generate_secret_key(&conn_2).expect("storage 2 load/generate");

        assert_ne!(
            key_1.public(),
            key_2.public(),
            "независимые storage должны получать разные identity"
        );
    }

    #[test]
    fn secret_key_is_written_to_sync_kv_after_first_call() {
        let conn = setup_db();

        assert_eq!(
            crate::db::get_sync_kv(&conn, "iroh.secret_key").unwrap(),
            None,
            "до первого вызова ключа в sync_kv быть не должно"
        );

        let _ = load_or_generate_secret_key(&conn).expect("first load/generate");

        let stored = crate::db::get_sync_kv(&conn, "iroh.secret_key").unwrap();
        assert!(
            stored.is_some(),
            "после первого вызова secret key должен быть записан в sync_kv"
        );
    }
}
