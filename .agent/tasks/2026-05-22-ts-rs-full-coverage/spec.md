# ts-rs full coverage — Phase 5 bug-detection follow-up

## Контекст

Phase 5 pilot покрыл 3 core wire-type'а (`ArkObject`, `ObjectType`, `ObjectLink`)
через `ts-rs` codegen из Rust → TypeScript. Источник правды для wire-types
теперь `crates/ark-core/rust/src/types.rs`; bindings — в
`packages/ark/src/generated/`.

Цель этой задачи — расширить покрытие на оставшиеся ~10 Serialize+Deserialize
struct'ов в `types.rs`, чтобы устранить drift между Rust и TS типами по всему
wire-protocol'у (todos/projects/areas/tags/headings, usage-tracker, peers,
sync entities, analytics).

Часть 2 (типизированный `request<Op>` в `ark-client.ts`) — опционально,
отложена на отдельную итерацию: проще выкатить полное покрытие codegen'а
и далее мигрировать клиентский API.

## Scope

- ✅ Добавить `#[cfg_attr(feature = "ts-rs", derive(TS))]` + export на ВСЕ
  Serialize+Deserialize struct'ы в `crates/ark-core/rust/src/types.rs`.
- ✅ Включить feature `import-esm` в ts-rs, чтобы кросс-импорты в generated
  файлах содержали `.js` (требование NodeNext в `packages/ark`).
- ✅ Обновить `packages/ark/src/generated/index.ts` и
  `packages/ark/src/index.ts` — re-export всех новых типов.
- ✅ Добавить `packages/ark/src/generated/**` в `.oxfmtrc.json`
  `ignorePatterns` — ts-rs пишет файлы без форматирования, и лефтхук-гвард
  `ts-rs-bindings-fresh` регенерирует working tree в нерасформатированный
  вид. Чтобы гвард и oxfmt не воевали, generated/ исключён из форматирования
  (стандартная практика для генерированного кода).
- ❌ (out of scope) Часть 2 — типизированный `request<Op>` через `ArkOpMap`.
  Большой объём работы (десятки operations), хочется сделать отдельным PR
  после того как codegen стабилизируется.
- ❌ (out of scope) Миграция существующих ручных TS типов в `ark-client.ts`
  на generated — отдельная задача.

## Acceptance Criteria

1. **AC1**: `cargo test --features ts-rs --manifest-path crates/ark-core/rust/Cargo.toml --lib export_bindings` → 26 export*bindings*\* тестов PASS.
2. **AC2**: `bun run --cwd packages/ark typecheck` → PASS (нет TS ошибок).
3. **AC3**: `bun run --cwd shell typecheck` → PASS (нет регрессий).
4. **AC4**: `bunx oxlint .` → 0 errors (warnings допустимы — pre-existing).
5. **AC5**: `bunx oxfmt --check .` → clean.
6. **AC6**: `packages/ark/src/generated/` содержит файлы для всех 26
   exported типов (ArkObject, ObjectType, ObjectLink, TodoItem, Project,
   Area, Tag, Heading, TrackedApp, UsageSession, UsageEvent, UsageSummary,
   DailyTrendPoint, HourlyHeatmapCell, TopAppEntry, RecentSessionEntry,
   UsageAnalyticsSnapshot, UsageProcessCandidate, UsageGamePlaytimeBinding,
   UsageGamePlaytimeAggregate, UsageGameDailyTotal, UsageGameRangeTotal,
   UsageGamePlaytimeSummary, LoadAllData, SyncEntity, PeerRecord).

## Known issues / notes

- ts-rs выдаёт warning'и про `#[serde(default, skip_serializing_if =
"Option::is_none")]` — атрибут парсится не полностью. На output это не
  влияет: `Option<bool>` корректно становится `boolean | null`.
- `SyncEntity.data` (`serde_json::Map<String, Value>`) и `*.meta_json` (`Value`)
  типизированы как `Record<string, unknown>` через `#[ts(type = ...)]`.
- Pilot files (`ArkObject.ts`/`ObjectType.ts`/`ObjectLink.ts`) в HEAD были
  закоммичены **расформатированными** (после oxfmt). Чтобы соответствовать
  ts-rs raw output и не конфликтовать с лефтхук-гвардом, файлы теперь
  unformatted; `.oxfmtrc.json` явно игнорирует `packages/ark/src/generated/**`.

## Не сделано / отложено

- Playwright `tests/e2e/eden.spec.ts` не прогонялся — изменения чисто
  типовые (additive `#[derive(TS)]`), runtime пути не затронуты. Pre-push
  hooks прогонят `cargo nextest` и clippy.
- Часть 2 (typed `request<Op>`) — отдельный PR.
