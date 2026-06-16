//! Iroh p2p QUIC transport — Фаза 0 spike (GREEN stage).
//!
//! Зеркалит публичную форму `relay_transport.rs` (`RelayConfig`/`RelayEvent`/
//! `RelayTransport`), чтобы фаза 1 могла относительно механически встроить
//! этот транспорт в `mesh.rs`. См. spec:
//! `.agent/tasks/2026-06-16-iroh-transport/spec.md`.
//!
//! Реализация фазы 0: один прямой peer (без discovery/pairing UI).
//! `Endpoint::builder` строится через `presets::Minimal`, который не тащит
//! address lookup (DNS/pkarr), но по умолчанию даёт `RelayMode::Disabled`.
//!
//! Dev-flow cross-network шаг: `IrohConfig::relay_mode` управляет этим —
//! `None` (все production call sites) резолвится в `start()` в
//! `RelayMode::Default` (n0 production relay servers), нужный для NAT
//! traversal между двумя реальными машинами за разными NAT/firewall.
//! `Some(RelayMode::Disabled)` остаётся только у offline-тестов (нужно для
//! CI без сетевого доступа) — см. `tests/iroh_round_trip.rs`.
//!
//! Device-id маппинг — фаза 0 упрощение: на accept-стороне `from_device_id`
//! читается из самого сообщения (`protocol::message_origin_device_id`), а не
//! из EndpointId пира — полноценный device_id ↔ EndpointId реестр (нужен для
//! пар "кто это вообще такой до получения первого сообщения") — это фаза 1.
//!
//! Доступен только под feature `iroh-spike` — production build (`cargo build`
//! без флагов) не подтягивает крейт `iroh` и не компилирует этот файл.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Mutex;

use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode};
use iroh_tickets::endpoint::EndpointTicket;
use iroh_tickets::Ticket;
use rusqlite::Connection;
use tokio::sync::mpsc;

use crate::db::{get_sync_kv, set_sync_kv};
use crate::protocol::{
    deserialize_message, message_origin_device_id, serialize_message, LanSyncMessage,
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
    /// извлекается лениво при первом `send()`. Если задан `peer_addr`, он
    /// имеет приоритет (явный адрес важнее тикета на случай, если задано
    /// и то, и другое).
    pub peer_ticket: Option<String>,
    /// Dev-flow cross-network step: relay mode override for `start()`.
    /// `None` => production default — `RelayMode::Default` (n0 relay
    /// servers), needed for real NAT traversal between two machines.
    /// `Some(RelayMode::Disabled)` is for tests only (`iroh_round_trip.rs`):
    /// offline loopback on one machine must not depend on network/relay
    /// availability in CI. Production call sites (`main.rs::handle_start_sync`,
    /// `ffi.rs`) leave this `None`.
    pub relay_mode: Option<RelayMode>,
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
/// первом входящем `Hello` от соединения (см. accept-loop в `start()`) —
/// до этого момента `EndpointId` пира известен, но какой это CRDT-девайс —
/// нет (см. doc-comment модуля, открытый вопрос §4.3/фаза 1 в исходной
/// версии файла).
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

/// Handles connection lifecycle for the iroh p2p transport. См. модульную
/// документацию — реализация отложена на GREEN-стадию.
pub struct IrohTransport {
    config: IrohConfig,
    /// Запущенный `Endpoint` после `start()` — нужен для `endpoint_id()`/
    /// `endpoint_addr()`/`send()`. `None` до старта.
    endpoint: Mutex<Option<Endpoint>>,
    /// Shutdown signal sender (по аналогии с `RelayTransport::stop_tx`).
    stop_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    /// Шаг 3: device_id ↔ EndpointId реестр, заполняется по входящим Hello.
    /// `Arc`, потому что accept-loop живёт в спавненной `'static`-задаче и не
    /// может держать `&self` транспорта.
    registry: std::sync::Arc<DeviceRegistry>,
}

impl IrohTransport {
    pub fn new(config: IrohConfig) -> Self {
        IrohTransport {
            config,
            endpoint: Mutex::new(None),
            stop_tx: Mutex::new(None),
            registry: std::sync::Arc::new(DeviceRegistry::new()),
        }
    }

    /// Текущий снимок реестра device_id ↔ EndpointId (для тестов/диагностики).
    #[doc(hidden)]
    pub fn registry(&self) -> &DeviceRegistry {
        &self.registry
    }

    /// Тестовый хук: то же кодирование, что использует `our_ticket()`, но
    /// без необходимости поднимать реальный `Endpoint` — позволяет unit-тесту
    /// проверить чистый round-trip кодек тикета.
    #[doc(hidden)]
    pub fn ticket_string_for_addr(addr: &EndpointAddr) -> String {
        ticket_string(addr)
    }

    /// Возвращает нашу ticket-строку — то, что нужно передать другому
    /// устройству для pairing вместо сырого `EndpointAddr` (см.
    /// `IrohConfig::peer_ticket`/`from_ticket`). Требует, чтобы транспорт
    /// уже был запущен.
    ///
    /// Cross-network step (dev-flow pairing code): когда relay включён
    /// (`config.relay_mode` не `Some(RelayMode::Disabled)`, т.е. production
    /// путь), `endpoint_addr()`'s hardcoded `127.0.0.1` бесполезен другому
    /// физическому хосту — тикет должен нести relay URL, который iroh
    /// заполняет в `Endpoint::addr()` только после того, как endpoint
    /// "online" (см. `Endpoint::online()`/`watch_addr()` doc). Поэтому здесь
    /// ждём `online()` и берём `endpoint.addr()` напрямую, а не
    /// `endpoint_addr()` (который остаётся loopback-only хелпером для теста
    /// `iroh_round_trip.rs`, где relay явно `Disabled`).
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

    /// Резолвит эффективный адрес пира для `send()`: явный `peer_addr` имеет
    /// приоритет, иначе парсим `peer_ticket` (см. `IrohConfig` doc-comment).
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
    /// Нужен тесту, чтобы передать адрес одного узла другому без discovery.
    pub fn endpoint_id(&self) -> Option<EndpointId> {
        self.endpoint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|ep| ep.id())
    }

    /// Возвращает локальный адрес транспорта (для ручной передачи пиру в
    /// фазе 0, без pairing UI/discovery). Использует `bound_sockets()` +
    /// явную подстановку `127.0.0.1` — см. модульную документацию и
    /// `iroh_loopback_smoke.rs`: `endpoint.addr()` (watcher) в офлайн-режиме
    /// ненадёжен, `bound_sockets()` отдаёт реальный забинженный порт сразу.
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
}

#[async_trait::async_trait]
impl SyncTransport for IrohTransport {
    /// Запускает транспорт: байндит `Endpoint`, поднимает accept-loop для
    /// входящих bi-стримов и (если `config.peer_addr` задан) исходящее
    /// соединение. События доставляются через `event_tx` — аналог
    /// `RelayTransport::start`.
    ///
    /// Relay mode: `config.relay_mode` управляет NAT traversal. Production
    /// call sites (`main.rs::handle_start_sync`, `ffi.rs`) оставляют его
    /// `None`, что здесь резолвится в `RelayMode::Default` (n0 production
    /// relay servers) — без этого два узла за разными NAT/firewall не могут
    /// соединиться через интернет, только в одной LAN/loopback. Тест
    /// `iroh_round_trip.rs` явно передаёт `Some(RelayMode::Disabled)`, чтобы
    /// остаться offline (CI без сетевого доступа к relay).
    async fn start(&self, event_tx: mpsc::UnboundedSender<TransportEvent>) -> Result<(), String> {
        let relay_mode = self.config.relay_mode.clone().unwrap_or(RelayMode::Default);

        // Builder — ТОЛЬКО presets::Minimal (см. модульную документацию и
        // iroh_loopback_smoke.rs): empty() падает без crypto_provider, N0
        // тянет DNS/pkarr lookup даже с relay Disabled.
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

        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
        *self.stop_tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(stop_tx);

        // Accept-loop: каждое входящее соединение читает один framed
        // LanSyncMessage и эмитит MessageReceived. Фаза 0: один прямой peer,
        // поэтому цикл просто продолжает принимать (на случай нескольких
        // стримов/реконнектов в рамках теста), пока не придёт stop-сигнал.
        let accept_endpoint = endpoint.clone();
        let accept_event_tx = event_tx.clone();
        let accept_registry = self.registry.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        return;
                    }
                    incoming = accept_endpoint.accept() => {
                        let Some(incoming) = incoming else {
                            // Endpoint закрыт.
                            return;
                        };
                        let event_tx = accept_event_tx.clone();
                        let registry = accept_registry.clone();
                        tokio::spawn(async move {
                            let conn = match incoming.await {
                                Ok(conn) => conn,
                                Err(e) => {
                                    eprintln!("[IrohTransport] incoming connection failed: {e}");
                                    return;
                                }
                            };
                            let remote_endpoint_id = conn.remote_id();

                            let (_send, mut recv) = match conn.accept_bi().await {
                                Ok(streams) => streams,
                                Err(e) => {
                                    eprintln!("[IrohTransport] accept_bi failed: {e}");
                                    return;
                                }
                            };

                            match read_frame(&mut recv).await {
                                Ok(raw) => {
                                    if let Some(msg) = deserialize_message(&raw) {
                                        // Шаг 3: на Hello учим реестр (EndpointId
                                        // соединения -> CRDT device_id из payload) —
                                        // это единственное сообщение, которое
                                        // достоверно несёт device_id отправителя
                                        // независимо от заполненности
                                        // origin_device_id. Для остальных типов
                                        // сообщений сперва пробуем
                                        // origin_device_id из самого сообщения,
                                        // а если его нет — резолвим по реестру
                                        // через EndpointId этого соединения (а не
                                        // пустую строку — см. doc-comment
                                        // `DeviceRegistry`).
                                        if let LanSyncMessage::Hello { device_id, .. } = &msg {
                                            registry.insert(remote_endpoint_id, device_id.clone());
                                        }
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
                                    eprintln!("[IrohTransport] read_frame failed: {e}");
                                }
                            }
                        });
                    }
                }
            }
        });

        // Если задан peer_addr, мы не открываем соединение заранее — `send()`
        // коннектится лениво при первой отправке (см. doc-comment send()).
        let _ = &self.config;

        Ok(())
    }

    /// Отправляет сообщение через активный QUIC bi-стрим к пиру. Сигнатура
    /// не требует активного соединения как предусловие — реализация лениво
    /// коннектится к `config.peer_addr` при каждом вызове (фаза 0: один
    /// прямой peer, без connection pooling/outbox — это решение фазы 1, см.
    /// spec §3.4).
    ///
    /// `send()` остаётся синхронным по сигнатуре (как `RelayTransport::send`):
    /// фактическое соединение и запись выполняются в спавненной задаче
    /// (fire-and-forget), ошибки логируются в stderr, а не возвращаются
    /// вызывающему — `Ok(())` здесь означает "сообщение поставлено в очередь
    /// на отправку", а не "доставлено".
    fn send(&self, msg: LanSyncMessage) -> Result<(), String> {
        let endpoint = self
            .endpoint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| "iroh transport: send() called before start()".to_string())?;

        let peer_addr = self.resolve_peer_addr()?;

        tokio::spawn(async move {
            let conn = match endpoint.connect(peer_addr, ARK_SYNC_ALPN).await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("[IrohTransport] connect to peer failed: {e}");
                    return;
                }
            };

            let (mut send, mut recv) = match conn.open_bi().await {
                Ok(streams) => streams,
                Err(e) => {
                    eprintln!("[IrohTransport] open_bi failed: {e}");
                    return;
                }
            };
            let _ = &mut recv; // фаза 0: ответный поток не используется

            let payload = serialize_message(&msg);
            if let Err(e) = write_frame(&mut send, payload.as_bytes()).await {
                eprintln!("[IrohTransport] write_frame failed: {e}");
                return;
            }
            if let Err(e) = send.finish() {
                eprintln!("[IrohTransport] finish send stream failed: {e}");
                return;
            }
            // Дожидаемся, что receiver реально дочитал стрим, прежде чем
            // отпустить `conn` (drop закрывает соединение немедленно и может
            // гонкой обрезать ещё не прочитанные байты на стороне приёмника —
            // именно так и было: `accept_bi` на B падал с "closed by peer").
            if let Err(e) = send.stopped().await {
                eprintln!("[IrohTransport] waiting for stream stop failed: {e}");
            }
        });

        Ok(())
    }

    /// Сигнализирует фоновому accept-loop остановиться и закрывает `Endpoint`.
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
/// Это даёт стабильный `EndpointId` между рестартами процесса — без этого
/// каждый старт `IrohTransport` получал бы новый случайный identity (см.
/// `IrohConfig::secret_key` doc-comment, открытый вопрос §4.3 в spec) и пир
/// не смог бы узнать устройство по `EndpointId` повторно.
///
/// Принимает `&Connection` напрямую (как `get_sync_kv`/`set_sync_kv` в
/// `db.rs`), а не `Arc<dyn StorageBackend>`: чтение/запись `sync_kv` в этом
/// крейте не выведены в `StorageBackend` trait — это узкие функции над
/// сырым `rusqlite::Connection`, и весь остальной код (`relay_sync.rs`,
/// `sync_client.rs`, `sync_server.rs`, тесты в `db.rs`) обращается к ним
/// так же напрямую. Эта функция повторяет тот же паттерн ради
/// консистентности, а не вводит новый.
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
// Tests — Фаза 1 шаг 1: постоянная identity.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests — Шаг 3: ticket-based pairing + device_id ↔ EndpointId реестр (RED).
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
