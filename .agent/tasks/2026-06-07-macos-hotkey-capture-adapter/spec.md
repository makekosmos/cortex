# 2026-06-07 macos-hotkey-capture-adapter

## Context

Назначение хоткея диктации идёт через backend-op `dictation.begin_hotkey_capture`
(adapter-метод). Windows-ветка запускает low-level hook; **macOS-ветка —
заглушка `Err("begin_hotkey_capture: Windows-only (Phase 1)")`**. Чтобы UI не
падал, временно вкручено `:external-capture="isWindows"` в `DictationTab.vue` —
платформенный шов протёк в презентацию, нарушив adapter-first паттерн. Задача —
заполнить variation point нативным Swift-хелпером и откатить UI-leak.

## Scope

В задаче:

- Swift helper `capture-hotkey` (`platform/runtime/native/macos/capture-hotkey.swift`)
  — `CGEventTap` без фильтра по конкретному keyCode: ловит первый `keyDown` с
  ≥1 квалифицирующим модификатором, эмитит `{captured, keyCode, cmd, ctrl, alt,
shift, fn}`; `Escape` → `{cancelled}`. Parent-watchdog как в `hotkey-hold-monitor`.
- Регистрация helper'а в `HELPERS` (`macos_native.rs`) и в `build-macos-native.mjs`.
- `macos_native.rs`: `begin_capture(tx)` / `end_capture()`, конвертация
  mac `keyCode` → accelerator-строка (обратный маппинг к `mac_key_code`) и эмит
  `dictation_capture_key` (`{accelerator}`) / `dictation_capture_cancelled` в тот
  же broadcast-канал, что слушает `useDictationConfig.onDictationCaptureStart`.
- `host.rs`: заполнить `#[cfg(target_os = "macos")]` ветки
  `op_begin_hotkey_capture` / `op_end_hotkey_capture`.
- Фронт: `onDictationCaptureStart` предпочитает `e.accelerator` если есть
  (платформо-агностично); `DictationTab.vue` — безусловный `external-capture`,
  убрать `usePlatform()`/`isWindows`.

Не в задаче:

- Изменение Windows-ветки capture (остаётся vk-based).
- UI обработки permission-ошибки CGEventTap (helper при отказе → cancelled,
  UI не виснет, но отдельного тоста нет).
- Win+H audio picker race (отдельный долг).

## Дизайн-решение (отклонение от заметки)

Заметка предлагала helper эмитить `{vk, ctrl, shift, alt, win}`. Но macOS
`CGKeyCode` ≠ Windows VK: фронтовый `vkToKeyName(0)` для mac-кода 'a' вернёт
пусто → accelerator пустой → нажатие игнорится. Поэтому accelerator-строка
собирается **в Rust-адаптере** (там живёт mac-специфичный keyCode↔name), а в
broadcast уходит готовый `accelerator`. Фронт-ветка «если backend уже резолвнул
строку — используем её» платформо-агностична и не возвращает leak.

## Acceptance Criteria

AC1. `cargo test -p kepler-backend` зелёный, включая новые юнит-тесты обратного
маппинга `keyCode → accelerator` (буква, символ, cmd→Super, требование ≥1 mod).

AC2. `bun run --cwd platform/desktop build-macos-native` (или прямой запуск
`scripts/build-macos-native.mjs`) собирает `capture-hotkey` без ошибок swiftc;
бинарь появляется в `.tmp/native/macos/`.

AC3. `DictationTab.vue` не содержит `usePlatform`/`isWindows`; `external-capture`
безусловный. `vue-tsc` (typecheck) проходит.

AC4. macOS-ветки `op_begin_hotkey_capture` / `op_end_hotkey_capture` не
возвращают `Err("...Windows-only")`, а вызывают `macos_native::begin_capture` /
`end_capture`.

## Verification commands

- `cargo test -p kepler-backend` — AC1.
- `node platform/desktop/scripts/build-macos-native.mjs` — AC2.
- `grep -n "isWindows\|usePlatform" platform/desktop/src/views/settings/tabs/DictationTab.vue` пусто — AC3.
- `bun run --cwd platform/desktop typecheck` (vue-tsc) — AC3.
- Manual (pending, нет mac e2e): открыть Settings → Диктация на macOS, нажать
  «назначить хоткей», зажать Cmd+Shift+; — поле принимает сочетание; Escape —
  отмена. Заносится в `manual-tests-pending.md`.

## Out of scope decisions

- Помечаем долг `dictation.md` § Tech debt как закрытый после PASS.
