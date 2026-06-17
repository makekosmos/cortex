# Fix: локальная запись сущности не рассылает LiveChange (live-sync gap)

Дата: 2026-06-17
Классификация: FULL_LOOP (sync write boundary, transport-neutral).
Ветка: feat/iroh-transport.

## Симптом (наблюдаемый пользователем)

Синхронизация (iroh) работает только в момент установления соединения: при
коннекте две машины обмениваются изменениями (catch-up), но дальнейшие правки
«вживую» не доезжают. Отредактировал заметку на одной машине — на другой не
появляется, пока не переподключишься.

## Root cause (подтверждён чтением кода)

Это НЕ транспортный баг и НЕ idle-timeout (iroh keep-alive включён по умолчанию,
`QuicTransportConfigBuilder::new()` ставит `keep_alive_interval` +
`default_path_max_idle_timeout` — соединение живёт).

Реальная причина — в сайдкаре `ark-core-rpc` (`core/ark/crates/ark-core/rust/src/main.rs`):

- Обработчики локальной записи (`Request::UpsertObject`, `DeleteObject`,
  `UpsertObjectType`, `DeleteObjectType`, `UpsertObjectLink`, `DeleteObjectLink`,
  и legacy todo/project/area/tag/heading/tracked_app/usage_session/usage_event)
  пишут в БД через `db::*` и бампят version vector через
  `record_local_upsert` / `record_local_delete`, но **никогда не рассылают
  `LanSyncMessage::LiveChange`**.
- Единственный путь, рассылающий `LiveChange`, — отдельный RPC
  `Request::BroadcastChange` → `handle_broadcast_change` (main.rs:1446), который
  app-флоу записи объектов НЕ вызывает (см. `ark-client.ts` `objects.upsert` —
  дёргает только `upsert_object`, без `broadcastChange`).

Поэтому:

- **catch-up работает**: `record_local_upsert` → `bump_sync_version_vector`
  двигает VV в `sync_kv`; при (ре)коннекте `send_missing_entities` находит
  недостающую сущность по VV и шлёт `SyncChanges`.
- **live НЕ работает**: ни один `LiveChange` не уходит при локальной записи.

## Ключевые факты для реализации

- `DB` (используется `with_conn`) и `runtime.storage` (`SqliteStorageBackend`,
  строится из `get_shared_conn()`) — это ОДИН и тот же `Arc<StdMutex<Connection>>`.
  Запись через `with_conn` сразу видна `runtime.storage`. HLC consistency
  гарантирована: `record_local_upsert`→`bump_sync_version_vector` сохраняет HLC,
  который `load_entities`/`hlc_for` потом читают.
- `SyncEntity { entity_type, id, data, hlc, deleted }`. Для объекта catch-up
  строит `{ entity_type:"object", id, data: to_data_map(object), hlc: hlc_for(id),
deleted: None }` (db.rs ~3078-3088). Приёмник применяет через
  `storage.apply_entity` (relay_sync.rs `apply_entities`), для delete —
  `deleted: Some(true)` + tombstone (apply_entity_blocking, db.rs ~3109).
- `handle_broadcast_change` (main.rs:1446) рассылает в три адресата и это эталон
  fan-out: `runtime.server.broadcast_live_change(entity, None)` +
  по всем `runtime.clients` + `runtime.relay.broadcast_live_change(entity)`.
  ВАЖНО: он дополнительно `update_entity_hlc` + `storage.apply_entity` — для
  нашего пути это НЕ нужно (HLC уже проставлен, данные уже записаны). НЕ
  пере-штамповать HLC и НЕ пере-применять.

## Требуемое поведение

После УСПЕШНОЙ локальной записи сущности обработчик строит соответствующий
`SyncEntity` (с уже проставленным `record_local_*` HLC) и рассылает его как
`LiveChange` всем подключённым пирам — тем же fan-out, что `handle_broadcast_change`,
но без повторного штампа HLC и без повторного apply. Если sync-runtime не запущен
(`SYNC` = None) — тихий no-op (как сейчас при отсутствии транспорта).

Поведение должно быть transport-neutral (работает и для relay, и для iroh —
оба ходят через `runtime.relay` / `SyncTransport`).

## План реализации (ориентир, итоговое решение — на усмотрение исполнителя)

1. `record_local_upsert` вернуть HLC (как уже делает `record_local_delete`).
2. Helper `async fn broadcast_local_change(entity: SyncEntity)`:
   берёт `SYNC` (если `Some`), делает fan-out server+clients+relay БЕЗ
   apply/HLC-stamp. Для отсутствующего runtime — no-op.
3. Helper построения `SyncEntity` из (entity_type, id, serde-значение объекта,
   hlc, deleted). `to_data_map`-эквивалент: `serde_json::to_value(obj)` →
   object map. Для delete — `data` пустой, `deleted: Some(true)`, `hlc` от
   `record_local_delete`.
4. Вызвать в каждом local-write handler после успеха. Объектные обработчики —
   приоритет (Eden-заметки = объекты); для полноты и единообразия покрыть все
   `record_local_*`-обработчики.

## TDD (СТРОГО, RED first)

Перед имплементацией написать падающий тест и ПРОГНАТЬ его (увидеть RED):

- Тест уровня sidecar/runtime: поднять `SYNC` runtime с fake/loopback транспортом,
  который захватывает исходящие `LanSyncMessage`; выполнить `Request::UpsertObject`;
  ассертить, что был отправлен `LanSyncMessage::LiveChange` с `entity.id` == id
  объекта, `entity_type == "object"`, `deleted == None`, и HLC, совпадающим с тем,
  что в version vector. Аналогичный тест на `DeleteObject` → `deleted: Some(true)`.
- Сначала прогнать → убедиться, что падает (LiveChange не отправляется). Потом
  реализовать → green.
- Существующая инфраструктура: см. fake-transport паттерны в `relay_sync.rs`
  (mod tests) и loopback-тесты в `iroh_transport.rs`. Использовать
  `SyncTransport` trait с записью `send()` в общий буфер.

## Acceptance

- Новые тесты: RED до фикса, GREEN после.
- `cargo test -p ark-core` зелёный, без новых warnings.
- `cargo build -p ark-core --features iroh-spike` и без фичи — оба компилируются.
- Прод-сборка без `iroh-spike` байт-эквивалентна по поведению (фикс
  transport-neutral, не за фичефлагом — это корректно: gap общий для relay/iroh).
- Никаких изменений схемы БД, никаких destructive migrations.
- Один логический коммит.

## Вне scope

- Удаление relay (отдельный шаг, после подтверждения live-sync на 2 машинах).
- Дедуп `handle_broadcast_change` vs новый helper (можно зашарить fan-out, но не
  обязательно в этом коммите).
