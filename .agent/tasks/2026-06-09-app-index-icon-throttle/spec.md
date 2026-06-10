# 2026-06-09 — App index icon extraction throttle

## Контекст

На холодном старте `AppIndex::rescan()` throttles icon extraction только после `source.discover()`, но Windows sources уже извлекают иконки внутри discovery:

- `StartMenuSource::discover()` вызывает `ensure_icon_for_lnk`.
- `UwpSource::discover()` вызывает `ensure_icon_for_uwp`.

Это сохраняет cold-start storm: `.lnk` parsing, `ExtractIconExW`, GDI bitmap extraction, PNG encode/write и UWP logo reads идут пачкой до throttled loop.

## Scope

В scope:

- убрать eager icon extraction из Windows Start Menu и UWP discovery;
- сохранить достаточно metadata, чтобы throttled `AppIndex::rescan()` мог извлечь те же иконки позже;
- сохранить SQLite cache compatibility через additive migration;
- добавить regression coverage для no-eager discovery / metadata roundtrip.

Не в scope:

- renderer-side visible-row lazy loading;
- новый IPC endpoint для single-icon fetch;
- изменение launcher layout/placeholder UI;
- изменение app ranking/search semantics.

## Acceptance Criteria

**AC1.** `StartMenuSource::discover()` и `UwpSource::discover()` не вызывают icon extraction/write helpers и возвращают apps без `icon_path` на cold cache.

**AC2.** `App` хранит `icon_source` metadata для lazy extraction: Start Menu app содержит `.lnk` path и target path, UWP app содержит package lookup metadata.

**AC3.** `AppIndex::rescan()` остаётся единственным bulk extraction path: extraction выполняется последовательно в background-priority blocking task и делает паузу между реальными extraction attempts.

**AC4.** Existing app-index SQLite cache мигрирует additive и round-trip сохраняет `icon_source`, не ломая старые rows без metadata.

**AC5.** Regression tests/guards проходят: targeted Rust tests для app_index и relevant docs checks.
