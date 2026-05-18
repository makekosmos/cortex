# Глоссарий

## A

**ADR** (Architecture Decision Record) — документированное архитектурное решение. См. [Журнал решений](/reference/decisions).

**ARK** — local-first data runtime Kosmos. Rust + SQLite + sync. См. [ark-core](/packages/ark-core).

**`ark-core-rpc`** — канонический бинарь sidecar ARK runtime. Принимает JSON-RPC по stdin/stdout.

**`auth_secret`** — общий секрет space'а для HMAC-аутентификации пиров при sync.

## B

**Beacon** — UDP broadcast для discovery пиров в LAN. `crates/ark-core/rust/src/beacon.rs`.

## C

**CRDT** — Conflict-free Replicated Data Type. ARK использует HLC + version vectors для merge-семантики.

## D

**Direct writer** — Rust-процесс, который пишет напрямую в ARK SQLite через `ark_core::db` хелперы (не через RPC). Пример: `services/kepler-backend/src/usage_tracker`. **Обязан** обновлять `lan_sync.version_vector`.

## E

**Eden Heart** — Rust sidecar внутри Eden для полнотекстового поиска через Tantivy. См. [Eden](/apps/eden).

**Electron main** — главный процесс Electron-приложения. Только он имеет доступ к ARK и Node API. См. [Архитектура](/concepts/architecture).

## F

**FTS5** — SQLite-расширение полнотекстового поиска. ARK использует для `objects.search()` когда доступно, fallback на in-memory matching.

## H

**HLC** (Hybrid Logical Clock) — гибрид физического и логического времени. Каждое изменение получает HLC-метку для строгого порядка событий. `crates/ark-core/rust/src/hlc.rs`.

**HMAC peer auth** — опциональная аутентификация пиров через HMAC-SHA256 поверх `auth_secret`. **Не** шифрует трафик.

## I

**Injected sidecar** — режим `@kosmos/ark`, когда sidecar уже владеется другим слоем, и SDK получает `requestFn` / `onEventFn`. См. [@kosmos/ark](/packages/ark).

## K

**`@kosmos/ark`** — канонический TS SDK для ARK runtime.

**`@kosmos/visuals`** — общая UI-система (токены, тема, компоненты). См. [kosmos-visuals](/packages/visuals).

## L

**LAN sync** — peer-to-peer синхронизация в локальной сети через WebSocket, без relay.

**Local-first** — архитектурный принцип: данные живут на устройстве, синхронизация — поверх.

## N

**`note_obj`** — `object_type` для заметок Eden.

## O

**Object model** — generic object model ARK: `object_types`, `objects`, `object_links`. См. [Модель данных](/concepts/ark-objects).

## P

**Pinia** — store-менеджер Vue. Используется в Eden (`useEdenStore`, `useLayoutStore`).

**Preload API** — узкий API между renderer и main процессами Electron. Renderer обращается к нему через `window.<app>Api`.

**Proof loop** — формальный цикл `spec → evidence → verify → problems` для substantial-задач. См. [Proof loop](/concepts/proof-loop).

## R

**Relay** — WebSocket-сервер `ark-relay-server` (`services/ark-relay-server`), посредник между пирами через NAT.

**Renderer** — процесс Electron, рендерящий UI. Не имеет прямого доступа к SQLite или ARK.

**RPC** — JSON-RPC поверх stdin/stdout `ark-core-rpc`.

## S

**Self-managed sidecar** — режим `@kosmos/ark`, когда `ArkClient` сам спавнит и владеет процессом `ark-core-rpc.exe`.

**Sidecar** — отдельный процесс рядом с приложением. Примеры: `ark-core-rpc` (для всех Electron apps), Eden Heart (для Eden).

**Space** — отдельное пространство данных ARK. Разные spaces — разные SQLite-БД (`%APPDATA%\Kosmos\spaces\<spaceId>\ark.db`).

**Squircle** — закруглённый прямоугольник с переменной кривизной угла. Эстетика kosmos-visuals.

**Sync** — синхронизация состояния между пирами. См. [Sync](/concepts/sync).

**`sync_kv`** — ARK таблица для произвольного key/value (включая `lan_sync.version_vector` под фиксированным ключом).

**`sync_tombstones`** — durable tombstones для propagation удалений между пирами.

## T

**Tantivy** — Rust-библиотека полнотекстового поиска, используется в Eden Heart.

**`task_obj`** — `object_type` для задач Delphi.

**TipTap** — rich-text editor, используется в Eden.

**Tombstone** — запись об удалении сущности, нужна для propagation удалений на пиры.

**`tracked_apps`**, **`usage_sessions`**, **`usage_events`** — ARK usage-таблицы. Пишутся `services/kepler-backend/src/usage_tracker` напрямую через `ark_core::db`.

**Typed note** — заметка Eden с собственным `object_type` (не дефолтный `note_obj`). Имеет специализированный header и schema.

## U

**UniFFI** — Mozilla-инструмент для генерации FFI-обвязок Rust → Kotlin/Swift. `crates/ark-core/rust/src/ffi.rs`.

## V

**Vapor mode** — режим компиляции Vue 3.6 без VDOM. Eden использует Vapor для leaf-компонентов. TipTap-компоненты остаются в обычном VDOM режиме (interop через `vaporInterop: true`).

**Version vector** — векторное clock пира для CRDT merge. Хранится в `sync_kv` под ключом `lan_sync.version_vector`. Обновляется при каждой записи в синхронизируемую сущность.

**`@kosmos/visuals`** — общая UI-система. См. [kosmos-visuals](/packages/visuals).

## Z

**Zed Mono** — моноширинный шрифт, используется в `@kosmos/visuals` для `font-family-mono`.

