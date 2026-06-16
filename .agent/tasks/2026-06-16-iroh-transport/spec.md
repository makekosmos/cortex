# Spec: iroh как третий транспорт sync (фаза 0 — spike)

Статус: draft / фаза 0.
Дата: 2026-06-16.
Контракт CRDT (`protocol.rs`, `types.rs`) — не трогаем.

## 1. Проблема и мотивация

Сейчас в `core/ark/crates/ark-core/rust/src/` есть два транспорта, доставляющих
одни и те же сериализованные `LanSyncMessage` (JSON-строки, см.
`protocol::serialize_message`/`deserialize_message`, `PROTOCOL_VERSION = 1`)
между `device_id`:

- **LAN** (`mesh.rs` + `net.rs` + `beacon.rs`) — discovery в локальной сети и
  прямой TCP-коннект на порту `LAN_SYNC_PORT = 21531`. Работает только когда
  оба устройства в одной L2-сети (один Wi-Fi/роутер). Не работает через
  интернет, не работает за NAT между разными сетями.
- **Relay** (`relay_transport.rs`) — WebSocket-клиент к нашему собственному
  relay-серверу (`RelayConfig { url, space_id, device_id, device_name,
api_key, auth_secret }`). Решает проблему «разные сети», но:
  - требует, чтобы мы держали и оплачивали relay-сервер;
  - весь трафик идёт через этот сервер (более высокая задержка, единая точка
    отказа, доверие к серверу хотя бы на уровне metadata — кто с кем
    синхронизируется и когда);
  - нет prямого p2p — relay всегда посередине, даже если оба устройства могли
    бы соединиться напрямую.

[iroh](https://www.iroh.computer/) — Rust-библиотека для p2p QUIC-соединений
между узлами, адресуемыми по `NodeId` (ed25519 public key), а не по IP:port.
Ключевые свойства, релевантные нам:

- NAT hole-punching (попытка прямого p2p-коннекта через STUN-подобный
  механизм) с автоматическим **фоллбэком на relay**, если прямое соединение
  не удалось — то есть похожий fallback-паттерн на наш Relay, но встроенный в
  библиотеку и не требующий, чтобы relay-сервер был «нашим» (n0 держит
  бесплатные публичные relay; можно self-host свой).
  - QUIC = шифрование на транспортном уровне «из коробки» (TLS 1.3 поверх
    QUIC), то есть мы получаем e2e-шифрование транспортного канала без
    собственной криптографии.
  - После рефакторинга n0 core-крейт `iroh` — это **только** транспорт:
    `Endpoint`, ALPN-протоколы, bidirectional/unidirectional QUIC-стримы.
    `iroh-docs` (CRDT documents), `iroh-gossip`, `iroh-blobs` — отдельные
    крейты, которые нам не нужны (у нас уже есть свой CRDT-слой в `protocol.rs`
  * `types.rs`).

Итого мотивация: дать пользователям sync через интернет/разные сети **без**
необходимости поднимать или оплачивать наш relay-сервер на 100% случаев,
сохранив relay как явный fallback там, где iroh не пробьётся (corporate
firewalls, симметричный NAT без relay-доступа и т.п. — хотя iroh сам падает
на relay в этом случае).

## 2. Не-цели (явно вне фазы 0, и часть — вне scope этой фичи вообще)

- **Не трогаем CRDT-контракт.** `protocol.rs` (HLC, `VersionVector`,
  `compute_vector_diff`, батчинг, `LanSyncMessage`) и `types.rs` остаются
  ровно как есть. iroh — только новый способ доставить те же JSON-фреймы.
- **Не заменяем и не выключаем relay.** `relay_transport.rs` и `mesh.rs`
  остаются нетронутыми. iroh встаёт третьим транспортом рядом, не вместо.
- **Не делаем pairing-UI.** Никакого экрана в Eden/Delphi/настройках, никакого
  QR-кода, никакого `NodeTicket`-обмена через UI в этой фазе.
- **Не решаем `device_id` ↔ `NodeId` маппинг.** В фазе 0 оба participant'а
  теста жёстко знают `NodeId` друг друга (hardcoded в тесте/коде спайка) —
  открытие "как узнать NodeId пира в реальной жизни" откладывается на фазу 1.
- **Не делаем persisted identity.** `SecretKey` генерируется в памяти на
  старте процесса (как делает iroh by default), не сохраняется в ARK SQLite.
  Соответственно `NodeId` устройства меняется при каждом перезапуске — это
  нормально для spike, неприемлемо для продакшена (см. открытые вопросы).
- **Не интегрируем в `MeshCoordinator`/`mesh.rs` и не подключаем к
  `FfiSyncConfig`.** Фаза 0 — отдельный модуль и отдельный integration-тест,
  не вызывается из продакшен-пути `ArkCore::start_sync`. Веткуется только
  `#[cfg(feature = "iroh-spike")]` (имя флага — см. раздел 3).
- **Не оцениваем итоговую судьбу relay** (заменит ли его iroh когда-нибудь) —
  это вопрос для отдельного ADR после фазы 1, не для фазы 0.

## 3. Архитектура

### 3.1 Расположение

Новый файл: `core/ark/crates/ark-core/rust/src/iroh_transport.rs`.

Регистрация модуля в `core/ark/crates/ark-core/rust/src/lib.rs` — добавить
`pub mod iroh_transport;` рядом с существующими `pub mod relay_transport;`,
`pub mod mesh;` и т.д. (сейчас там уже объявлены: `beacon`, `db`, `delphi`,
`events`, `ffi`, `hlc`, `host`, `mesh`, `net`, `pomodoro`, `protocol`,
`relay_sync`, `relay_transport`, `schema`, `space`, `sync_client`,
`sync_server`, `types`).

Весь модуль и зависимость от крейта `iroh` — за Cargo feature-флагом, рабочее
имя **`iroh-spike`** (зафиксировать имя на ревью; альтернатива — `iroh`, но
тогда может конфликтовать по смыслу с именем самого крейта-зависимости в
сообщениях об ошибках, поэтому в спеке предлагается `iroh-spike`).
Production-сборка (`cargo build` без флагов, и весь существующий UniFFI/RPC
biid) не подтягивает крейт `iroh` и не компилирует `iroh_transport.rs` — по
аналогии с тем как `ts-rs` уже сделан optional/feature-gated в этом крейте.

### 3.2 Контур: повтор формы `RelayTransport`/`RelayEvent`

Чтобы фаза 1 могла относительно механически встроить iroh в `mesh.rs` так же,
как сейчас встроен relay, новый модуль повторяет публичную форму
`relay_transport.rs`:

```
pub struct IrohConfig { ... }          // аналог RelayConfig
pub enum IrohEvent {                    // аналог RelayEvent
    Connected { device_id: String },    // здесь — скорее node_id, см. 4
    Disconnected { device_id: String },
    MessageReceived { from_device_id: String, msg: LanSyncMessage },
}
pub struct IrohTransport { ... }
impl IrohTransport {
    pub fn new(config: IrohConfig) -> Self;
    pub async fn start(&self, event_tx: mpsc::UnboundedSender<IrohEvent>) -> Result<(), String>;
    pub fn send(&self, msg: LanSyncMessage) -> Result<(), String>;
    pub fn stop(&self);
}
```

Точные поля `IrohConfig` для фазы 0 — минимальные (не зеркалим
`RelayConfig` 1:1, потому что у iroh нет понятия `space_id`/`api_key`/
`auth_secret` на транспортном уровне):

- `device_id: String` — наш текущий идентификатор, прокидывается как есть.
- `secret_key: iroh::SecretKey` (или `Option<...>`, генерируется если `None`)
  — для фазы 0 не персистится, см. §2/§4.
- `peer_node_addr: iroh::NodeAddr` (или `NodeId` + явный список адресов/relay
  url) — peer фазы 0 задаётся явно, не через discovery.

### 3.3 ALPN и направление соединений

- ALPN-идентификатор протокола: `b"ark-sync/1"` (по аналогии с
  `PROTOCOL_VERSION = 1` в `protocol.rs`; версионируем ALPN отдельно от
  CRDT/JSON-протокола, чтобы можно было эволюционировать транспорт без
  привязки к содержимому сообщений).
- **Приём (входящие соединения):** `Endpoint::bind()` с указанным ALPN →
  accept-loop: `endpoint.accept().await` → для каждого входящего соединения
  открываем bi-стрим, читаем фреймы, прогоняем через
  `protocol::deserialize_message`, эмитим `IrohEvent::MessageReceived`.
  Это прямой аналог accept-loop в `net.rs` (TCP) и серверной стороны
  `relay_transport.rs` (там сервер chужой, но цикл чтения тот же паттерн).
- **Отправка:** `endpoint.connect(peer_node_addr, ALPN).await` → открываем
  bi-стрим (или uni-стрим, если в фазе 0 достаточно одностороннего потока для
  каждого сообщения — решить во время реализации, какой дешевле при частых
  маленьких сообщениях) → `protocol::serialize_message` → пишем в стрим.
- Framing: QUIC-стримы — байтовый поток, не пакеты, поэтому, в отличие от
  WebSocket text-frame в `relay_transport.rs` (где один `Message::Text` =
  одно `LanSyncMessage`), на каждое сообщение нужен явный delimiter или
  length-prefix при записи в стрим. Зафиксировать в фазе 0 простейший вариант
  — newline-delimited JSON (`\n` после каждого `serialize_message`) — или
  length-prefixed frame (`u32` big-endian длина + JSON). Выбор — на
  реализацию, но **должен быть зафиксирован явно**, не "как получится",
  потому что протокол не даёт встроенного фрейминга сам.

### 3.4 Переиспользование идей outbox + backoff

`RelayTransport` держит `outbox: Arc<Mutex<VecDeque<LanSyncMessage>>>` (cap 500) для сообщений, отправленных пока транспорт не на связи, плюс
exponential backoff (1s → 60s, см. `relay_transport.rs:117-223`) на
reconnect-loop.

Для фазы 0 это **не обязательно** реализовывать полностью (spike — про
доказательство доставки фрейма, не про robustness), но дизайн `IrohTransport`
должен оставлять для этого место: `send()` не должен требовать активного
соединения как предусловие (та же сигнатура `Result<(), String>`, тот же
паттерн — если стрим не открыт, либо лениво коннектимся, либо складываем в
outbox). Полный backoff-цикл реализуется в фазе 1 вместе с persisted
identity и реальным reconnect.

## 4. Открытые вопросы для будущих фаз (решение пользователя нужно не сейчас, но явно зафиксировать, что они есть)

1. **`device_id` ↔ `NodeId` маппинг.** Сейчас `device_id` — наша строка
   (видна в `Hello`, `HLC`, дедупе в `mesh.rs`). `NodeId` — ed25519 public
   key. Нужно решить: храним ли `NodeId` как доп. поле в существующей
   peer-таблице ARK, или `device_id` становится производным от `NodeId`
   (тогда миграция существующих peer-записей)?
2. **Pairing-механизм.** iroh даёt `NodeTicket` (encode NodeId + adresses +
   relay url в один токен) — как пользователь передаёт тикет второму
   устройству? QR-код, share-ссылка, ручной ввод? Это отдельный UI-проект для
   фазы 2, но сама механика (что кодируем в тикет, TTL тикета, ротация)
   нужно решить раньше.
3. **Persisted `SecretKey`.** Если `NodeId` должен быть стабильным между
   перезапусками приложения (а это нужно для долгоживущих pairing-связей),
   `SecretKey` нужно хранить — где? Новая таблица в ARK SQLite? ОС keychain
   (Windows Credential Manager)? Кто отвечает за ротацию/revocation?
4. **Заменяет ли iroh relay в итоге, или они живут параллельно навсегда?**
   Сейчас relay — единственный путь между разными сетями. Если iroh покрывает
   это лучше (direct p2p + автоматический relay-fallback от n0), стоит ли
   через несколько фаз депрекейтить собственный `relay_transport.rs`, или
   оставить его как «независимый от n0 инфраструктуры» fallback (на случай
   если публичные relay n0 недоступны/заблокированы в регионе)? Это решение
   пользователя/архитектора после того, как фаза 1 покажет реальный
   success-rate hole-punching в целевых сетевых условиях.
5. **Вес зависимости.** `iroh` тянет `quinn` (QUIC-реализация) и связанный
   стек (rustls и т.п.). Нужно явно замерить прирост размера бинаря/времени
   компиляции после фазы 0 (см. риски, §7) и решить, приемлемо ли это для
   desktop-приложения, или нужен более легковесный путь.
6. **Точная версия крейта `iroh`.** Не зафиксирована в этом spec — нужно
   явно проверить актуальный релиз на crates.io/репозитории n0 на момент
   реализации фазы 0 (см. итоговое резюме); важно убедиться, что выбранная
   версия соответствует пост-рефакторингу "core = только транспорт" (а не
   старой версии, где `iroh` тянул docs/gossip/blobs по умолчанию).

## 5. Фазовый план

### Фаза 0 — spike за feature-флагом (этот spec)

- Добавить `iroh` (и при необходимости `iroh-net`/совместимый core-крейт —
  уточнить на этапе реализации, какой именно крейт даёт `Endpoint` после
  рефакторинга n0) под `[dependencies]` с `optional = true` + feature
  `iroh-spike = ["dep:iroh"]` в `core/ark/crates/ark-core/rust/Cargo.toml`,
  по аналогии с тем как там уже сделан `ts-rs`.
- Реализовать `iroh_transport.rs`: `IrohConfig`, `IrohEvent`,
  `IrohTransport::{new, start, send, stop}` — минимально достаточно для
  одного полного фрейма `LanSyncMessage` "туда" и подтверждения "обратно" в
  тесте.
  - НЕ интегрировать с `mesh.rs`/`FfiSyncConfig`/UniFFI — модуль
    самодостаточен, вызывается только из теста (и опционально из dev-bin /
    example, если потребуется для ручной проверки между двумя реальными
    машинами через интернет — см. ниже "ручная проверка").
- Доказать на интеграционном тесте (см. §6) round-trip двух iroh-узлов в
  одном процессе на loopback — аналог того, что `relay_round_trip.rs` делает
  для relay.
- Опционально (если время позволяет, не блокирует фазу 0): ручная проверка
  между двумя реальными машинами в разных сетях (например, через мобильный
  hotspot vs домашний Wi-Fi), чтобы увидеть hole-punching/relay-fallback в
  деле, а не только loopback. Не формализуется тестом, фиксируется как
  заметка в PR/задаче.

### Фаза 1 — реальный p2p за NAT

- Persisted `SecretKey` в ARK (схема — открытый вопрос §4.3).
- `device_id` ↔ `NodeId` маппинг (открытый вопрос §4.1) и его персистентность.
- Pairing через `NodeTicket` (программная часть — encode/decode, валидация,
  TTL) без UI (UI — фаза 2).
- Полный outbox + backoff в `IrohTransport`, на уровне `relay_transport.rs`.
- Реальный двусторонний sync между двумя устройствами за NAT (не loopback) —
  через `mesh.rs` интеграцию, как третий транспорт рядом с LAN/relay, с тем
  же dedup через `MeshCoordinator`.

### Фаза 2 — UI и итоговое решение про relay

- UI настроек в Eden/Delphi: статус iroh-соединения, ручка pairing (QR/ссылка
  на основе `NodeTicket`), индикатор direct p2p vs relay-fallback.
- Архитектурное решение по открытому вопросу §4.4 (судьба собственного
  relay) — ADR, не код.

## 6. План тестирования (SDD, RED → GREEN)

Первый тест фазы 0 — **интеграционный round-trip двух iroh-`Endpoint`**,
структурно зеркалящий существующие
`core/ark/crates/ark-core/rust/tests/relay_round_trip.rs` (relay) и
`sync_round_trip.rs` (полный sync поверх TCP):

Новый файл: `core/ark/crates/ark-core/rust/tests/iroh_round_trip.rs`,
помеченный `#[cfg(feature = "iroh-spike")]` / запускается через
`cargo test --features iroh-spike --test iroh_round_trip`.

Сценарий (по аналогии с `relay_round_trip.rs:121-201`):

1. Поднять `IrohTransport` A и `IrohTransport` B в одном процессе (в отличие
   от relay-теста, здесь не нужен отдельный "сервер" — это p2p, оба узла
   равноправны; единственная инфраструктура — публичный/тестовый relay для
   discovery, который предоставляет сам iroh, либо явная передача адресов
   друг другу в обход discovery, если фаза 0 решит не зависеть от внешней
   сети при прогоне теста в CI — см. ниже про CI-риск).
2. B стартует первым, печатает свой `NodeId`/`NodeAddr`.
3. A стартует, зная `NodeAddr` B (передан напрямую в конфиге теста — без
   pairing UI, без discovery поиска).
4. A вызывает `send(LanSyncMessage::LiveChange { entity: <тестовая entity>, .. })`
   — переиспользовать тот же паттерн тестовой entity, что в
   `relay_round_trip.rs:164-184` (`SyncEntity` с `entity_type: "todo"`, тот же
   формат `hlc`).
5. Тест ждёт `IrohEvent::MessageReceived` на стороне B с таймаутом (5s, как в
   relay-тесте) через `tokio::time::timeout`.
6. Ассерты: `entity.id`, `entity.entity_type` совпадают с отправленными —
   то есть **фрейм дошёл через iroh байт-в-байт и распарсился обратно в тот
   же `LanSyncMessage`**, без какого-либо участия CRDT-логики (это чисто
   транспортный тест, как и `relay_round_trip.rs`).

RED-стадия: написать этот тест и `iroh_transport.rs` с одними сигнатурами
(`todo!()`/`unimplemented!()` в `start`/`send`) — тест должен компилироваться
и упасть (паника либо таймаут на `MessageReceived`), прежде чем будет
реализована сама логика accept-loop/connect/send. Зафиксировать факт падения
(лог теста) перед тем как писать реализацию.

**Риск для CI:** если iroh по умолчанию использует публичную discovery
service/relay n0 для обмена адресами (даже на loopback), тест в CI без
доступа к интернету может зависнуть/упасть. Нужно на этапе реализации
проверить, можно ли тест держать полностью офлайн (передавая `NodeAddr` с
явными прямыми адресами в `bind_addr`/socket напрямую, без discovery) — если
нет, тест помечается `#[ignore]` по умолчанию и гоняется вручную/в отдельном
CI-job с доступом в интернет. Зафиксировать это решение в PR с тестом, не
тихо.

## 7. Риски

1. **Вес зависимости.** `iroh` → `quinn` (QUIC) → `rustls`/`ring` и связанный
   крипто-стек. Это первая QUIC-зависимость в крейте (`tokio-tungstenite`
   сейчас даёт WebSocket-over-TCP, не QUIC) — ожидаем заметный прирост
   времени компиляции и размера бинаря. Нужно измерить `cargo build --release`
   до/после на этом крейте при включённом `iroh-spike`, прежде чем решать
   про фазу 1 (где флаг станет default-on).
2. **Зависимость от внешней инфраструктуры n0.** Hole-punching/discovery в
   iroh по умолчанию полагается на публичные relay/discovery-серверы n0
   (если не настроен self-hosted relay). Это новая внешняя зависимость,
   которой не было ни у LAN, ни у нашего relay (наш relay — полностью под
   нашим контролем). Если n0 инфраструктура недоступна (down, заблокирована
   в регионе, deprecated в будущем) — iroh-транспорт деградирует. Снижается
   в фазе 1+ опцией self-host relay для iroh, но это доп. инфраструктура,
   которую мы тогда тоже обслуживаем — то есть может не дать обещанной
   экономии по сравнению с нашим текущим relay.
3. **Версия крейта `iroh` не зафиксирована в этом spec.** Экосистема активно
   меняется (n0 недавно выделил core/docs/gossip/blobs в отдельные крейты) —
   высокий риск, что pinned версия устареет к моменту реализации или что
   нужная "только-транспорт" форма API ещё не стабилизировалась в latest
   release. Перед началом реализации фазы 0 — обязательно сверить changelog
   крейта `iroh` на crates.io/GitHub n0-computer/iroh на дату реализации, а
   не доверять версии, названной в этом spec на дату его написания
   (2026-06-16).
4. **Фрейминг сообщений в QUIC-стриме** (см. §3.3) — у WebSocket/relay
   фрейминг бесплатный (text frame = одно сообщение), у raw QUIC-стрима его
   нужно реализовать руками. Риск рассинхронизации/частичного чтения если
   сделать наивно (просто `write_all(json_bytes)` без delimiter) — два
   сообщения, отправленных быстро подряд, могут склеиться на приёме.
5. **`unwrap()`/паника в новом коде.** Проект запрещает
   `Mutex::lock().unwrap()` без poison recovery в production Rust-путях
   (см. правило в CLAUDE.md). Хотя фаза 0 — spike за флагом, не входящий в
   production build, желательно сразу писать `IrohTransport` в стиле
   остального крейта (`unwrap_or_else(|e| e.into_inner())`, как в
   `relay_transport.rs`), чтобы фаза 1 не наследовала технический долг.

## Фаза 0 — РЕЗУЛЬТАТ (2026-06-16)

**Статус:** DONE. Доказано, что `LanSyncMessage` (наш CRDT-контракт) ездит поверх iroh-транспорта офлайн.

**Версия:** iroh 1.0.0 (релиз 2026-06-15).

**Созданные/изменённые файлы** (за фичей `iroh-spike`, production-сборка не затронута):

- `core/ark/crates/ark-core/rust/src/iroh_transport.rs` — `IrohConfig`/`IrohEvent`/`IrohTransport` с реальной iroh-логикой.
- `core/ark/crates/ark-core/rust/tests/iroh_round_trip.rs` — зелёный round-trip `LanSyncMessage::LiveChange`.
- `core/ark/crates/ark-core/rust/tests/iroh_loopback_smoke.rs` — отдельный smoke на чистый iroh API (де-риск).
- `Cargo.toml` (фича `iroh-spike = ["dep:iroh"]`), `lib.rs` (`#[cfg(feature="iroh-spike")] pub mod iroh_transport;`).

**Тест:** `cargo test -p ark-core --features iroh-spike --test iroh_round_trip` → ok (стабильно 4/4). Офлайн, без интернета/relay/discovery.

**Рабочий рецепт iroh 1.0.0** (важно для фазы 1, чтобы не переоткрывать грабли):

- **Билд:** `Endpoint::builder(presets::Minimal).relay_mode(RelayMode::Disabled).alpns(vec![b"ark-sync/1".to_vec()]).bind().await`. НЕ `Endpoint::empty()` (падает без crypto_provider). НЕ `presets::N0` (тянет DNS/pkarr lookup даже с выключенным relay).
- **Локальный адрес:** `endpoint.bound_sockets()` возвращает `0.0.0.0:PORT` (адрес биндинга) — для коннекта вручную подставить `127.0.0.1` с тем же портом. `endpoint.addr()` (watcher) офлайн ненадёжен.
- `EndpointAddr::new(id).with_ip_addr(socket)`.
- **Accept двухступенчатый:** `endpoint.accept().await` → `Option<Connecting>` → ещё `.await` → `Connection`; затем `conn.accept_bi().await`. Инициатор: `endpoint.connect(addr, ALPN).await` → `conn.open_bi().await`.
- **Framing самостоятельный:** 4-байтовый big-endian length prefix + JSON (`serialize_message`/`deserialize_message`). iroh stream — байтовый, framing из коробки нет.
- **ВАЖНО:** после `send.finish()` сделать `send.stopped().await` перед тем как отпустить `Connection`, иначе соединение дропается раньше, чем приёмник дочитает стрим (ошибка `closed by peer: 0`).

**Новая терминология iroh** (старое устарело): `EndpointAddr` (не NodeAddr), `EndpointId` (не NodeId), `endpoint.id()`, `endpoint.addr()`. QUIC-слой — `noq` (не quinn).

**Вес зависимости:** iroh тянет ~179 транзитивных крейтов, ~2 мин чистой сборки. Существенно для будущего решения «заменять relay или держать iroh опционально» (фаза 2).

**`from_device_id` в фазе 0** берётся из `message_origin_device_id(&msg)`; полноценный реестр `device_id`↔`EndpointId` — задача фазы 1.
