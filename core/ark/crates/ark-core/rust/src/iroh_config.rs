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
