//! Iroh p2p QUIC transport — Фаза 0 spike (GREEN stage).
//!
//! Зеркалит публичную форму `relay_transport.rs` (`RelayConfig`/`RelayEvent`/
//! `RelayTransport`), чтобы фаза 1 могла относительно механически встроить
//! этот транспорт в `mesh.rs`. См. spec:
//! `.agent/tasks/2026-06-16-iroh-transport/spec.md`.
//!
//! Реализация фазы 0: один прямой peer (без discovery/pairing UI), loopback
//! на одной машине без relay/internet (нужно для CI) — `Endpoint::builder`
//! строится через `Builder::empty()`, который по умолчанию уже даёт
//! `RelayMode::Disabled` и пустой address lookup; единственное, что нужно
//! добавить вручную — `crypto_provider` (по умолчанию `Builder::empty()` не
//! выбирает его за нас, в отличие от presets).
//!
//! Device-id маппинг — фаза 0 упрощение: на accept-стороне `from_device_id`
//! читается из самого сообщения (`protocol::message_origin_device_id`), а не
//! из EndpointId пира — полноценный device_id ↔ EndpointId реестр (нужен для
//! пар "кто это вообще такой до получения первого сообщения") — это фаза 1.
//!
//! Доступен только под feature `iroh-spike` — production build (`cargo build`
//! без флагов) не подтягивает крейт `iroh` и не компилирует этот файл.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Mutex;

use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode};
use rusqlite::Connection;
use tokio::sync::mpsc;

use crate::db::{get_sync_kv, set_sync_kv};
use crate::protocol::{
    deserialize_message, message_origin_device_id, serialize_message, LanSyncMessage,
};

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
}

#[derive(Debug)]
pub enum IrohEvent {
    Connected {
        device_id: String,
    },
    Disconnected {
        device_id: String,
    },
    MessageReceived {
        from_device_id: String,
        msg: LanSyncMessage,
    },
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
}

impl IrohTransport {
    pub fn new(config: IrohConfig) -> Self {
        IrohTransport {
            config,
            endpoint: Mutex::new(None),
            stop_tx: Mutex::new(None),
        }
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

    /// Запускает транспорт: байндит `Endpoint` на loopback без relay/discovery
    /// (`RelayMode::Disabled`), поднимает accept-loop для входящих
    /// bi-стримов и (если `config.peer_addr` задан) исходящее соединение.
    /// События доставляются через `event_tx` — аналог `RelayTransport::start`.
    pub async fn start(&self, event_tx: mpsc::UnboundedSender<IrohEvent>) -> Result<(), String> {
        // Builder — ТОЛЬКО presets::Minimal (см. модульную документацию и
        // iroh_loopback_smoke.rs): empty() падает без crypto_provider, N0
        // тянет DNS/pkarr lookup даже с relay Disabled.
        let mut builder = Endpoint::builder(presets::Minimal)
            .relay_mode(RelayMode::Disabled)
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
                        tokio::spawn(async move {
                            let conn = match incoming.await {
                                Ok(conn) => conn,
                                Err(e) => {
                                    eprintln!("[IrohTransport] incoming connection failed: {e}");
                                    return;
                                }
                            };

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
                                        // Фаза 0: from_device_id читается из самого
                                        // сообщения (origin_device_id), а не из
                                        // EndpointId пира — полноценный device_id
                                        // ↔ EndpointId реестр это фаза 1 (см.
                                        // модульную документацию).
                                        let from_device_id =
                                            message_origin_device_id(&msg).unwrap_or_default();
                                        let _ = event_tx.send(IrohEvent::MessageReceived {
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
    pub fn send(&self, msg: LanSyncMessage) -> Result<(), String> {
        let endpoint = self
            .endpoint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| "iroh transport: send() called before start()".to_string())?;

        let peer_addr = self
            .config
            .peer_addr
            .clone()
            .ok_or_else(|| "iroh transport: send() requires config.peer_addr".to_string())?;

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
    pub fn stop(&self) {
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
