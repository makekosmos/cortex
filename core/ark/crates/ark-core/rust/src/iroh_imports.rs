// Iroh p2p QUIC transport — Фаза 0 spike (GREEN stage — bidirectional).
//
// Зеркалит публичную форму `relay_transport.rs` (`RelayConfig`/`RelayEvent`/
// `RelayTransport`), чтобы фаза 1 могла относительно механически встроить
// этот транспорт в `mesh.rs`. См. spec:
// `.agent/tasks/2026-06-16-iroh-transport/spec.md`.
//
// ## Дизайн (split-task revision — fixes select! cancellation desync)
//
// Каждое QUIC-соединение (входящее или исходящее) живёт долго: один bi-стрим
// на всё время соединения, кадры идут back-to-back без finish() между ними.
// Это зеркалит `RelayTransport`: один WebSocket-стрим на соединение.
//
// ### Ключевое исправление: раздельные задачи для чтения и записи
//
// Предыдущая реализация использовала `tokio::select!`-loop, который владел
// ОБОИМИ половинами bi-стрима. `read_frame` НЕ является cancellation-safe:
// он делает `recv.read_exact(len_buf)` затем `recv.read_exact(payload)`.
// Когда ветка `out_rx.recv()` становилась ready ПОКА `read_frame` уже
// потребил 4-байтовый length-prefix но не payload — `select!` дропал
// `read_frame` future. Потреблённые байты терялись. Следующая итерация
// читала payload-байты как length-prefix → desync → bogus length → ошибка
// MAX_FRAME_LEN → соединение рвалось.
//
// Теперь `handle_connection` разделяет bi-стрим на два независимых task:
// - **writer task**: владеет `send`. Пишет Hello-фрейм, затем loop на
//   `out_rx.recv()` → `write_frame`. Никогда не конкурирует с reader.
// - **reader task**: владеет `recv`. loop на `read_frame` → десериализация
//   → emit MessageReceived. Никогда не прерывается writer'ом.
//
// Оба таска разделяют `tokio::sync::Notify` для взаимного teardown: если
// один завершается (ошибка / close stream), он нотифицирует другой.
//
// `send()` кладёт сообщение в `broadcast::Sender`, writer подписан.
// Если подписчиков нет (нет соединения) — сообщение молча дропается:
// пропущенные pre-connect live-changes скомпенсируются VersionVector
// обменом при (ре)коннекте.
//
// При подключении транспорт автоматически инжектирует аутентифицированный
// `Hello` как первый фрейм (аналогично `relay_transport.rs` строки 128-148).
// Это запускает CRDT-рукопожатие через `RelaySync::handle_message` без
// явного вызова `send(Hello)` сверху.
//
// Dial-loop (если `config.peer_addr/peer_ticket`) пересоединяется с
// экспоненциальным backoff (1s → 2s → ... → 60s) при обрыве — зеркалит
// RelayTransport.
//
// Shutdown: `tokio::sync::watch::channel(false)` позволяет нескольким задачам
// одновременно наблюдать сигнал остановки (в отличие от oneshot).
//
// Relay mode: `config.relay_mode` управляет NAT traversal. Production
// call sites (`main.rs::handle_start_sync`, `ffi.rs`) оставляют его
// `None`, что резолвится в `RelayMode::Default` (n0 production relay
// servers) — без этого два узла за разными NAT/firewall не могут
// соединиться. Тест `iroh_round_trip.rs` явно передаёт
// `Some(RelayMode::Disabled)`, чтобы остаться offline в CI.
//
// Доступен только под feature `iroh-spike` — production build (`cargo build`
// без флагов) не подтягивает крейт `iroh` и не компилирует этот файл.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode};
use iroh_tickets::endpoint::EndpointTicket;
use iroh_tickets::Ticket;
use rusqlite::Connection;
use tokio::sync::{broadcast, mpsc, watch, Notify};

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
