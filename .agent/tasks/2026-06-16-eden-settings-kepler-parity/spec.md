# Task Spec — Eden settings = Kepler settings (shared canon in @kosmos/visuals)

## Task ID / Path

- `2026-06-16-eden-settings-kepler-parity`
- `.agent/tasks/2026-06-16-eden-settings-kepler-parity/spec.md`

## Original task statement

> Пользователь: настройки Eden должны выглядеть РОВНО как настройки Kepler —
> те же компоненты, та же вёрстка, ничего не выдумывать. Решение по реализации:
> вынести общую обвязку страницы настроек Kepler в `@kosmos/visuals` и подключить
> и в Kepler, и в Eden.

## Context / current state

- Канон строк (`SettingsList`, `SettingsToggleRow`, `SettingsButtonRow`, `SettingsSidebar`, `SettingsSidebarButton`) уже в `@kosmos/visuals`; Eden их уже использует.
- Обвязка страницы Kepler (layout `.settings`/`.settings-shell`/`.settings-content`, `content-header` с chevron'ами, секции-капсы, типографика) живёт в `platform/desktop/src/views/settings/settings-shared.css` + inline-разметке `SettingsView.vue`. В `@kosmos/visuals` её нет.
- Eden — отдельный workspace, импортировать из `platform/desktop` не может. Поэтому общую обвязку выносим в `@kosmos/visuals`.
- Kosmos — Windows-only; mac-ветку (`SettingsTopTabs`) в Eden не тащим.

## Scope

In scope:

- Вынести ОБЩУЮ (не Kepler-специфичную) обвязку страницы настроек в `@kosmos/visuals`:
  - shared stylesheet с generic-классами shell/header/section/row/typography (verbatim из `settings-shared.css`);
  - компонент `SettingsContentHeader` (тонкий drag-бар с back/forward chrome) — единый для обоих.
- Переключить Kepler `SettingsView` на shared CSS + `SettingsContentHeader`, сохранив текущий визуал Kepler 1:1.
- Перестроить `EdenSettingsView` + tab-компоненты Eden на ту же обвязку и те же канон-компоненты (убрать кастомные карточки-секции, h1/h2, собственный `SettingsPage.css` page-shell).

Out of scope:

- Перенос Kepler-специфичных классов (stats-cards, security-page, hotkey, autostart, ttl) в visuals — остаются локальными в Kepler.
- Изменение НАБОРА настроек Eden (spellcheck / типы / markdown / trash / vim остаются) — меняется только обвязка/визуал, не содержание.
- ARK/data/sync/schema/write-boundary, command-bus.
- mac-специфичная вёрстка.

## Assumptions

- «Те же компоненты» = буквально общие компоненты/стили из `@kosmos/visuals`, а не копии.
- Kepler-визуал должен остаться идентичным после переключения на shared-источник (тот же CSS, просто перемещён).
- content-header в Eden повторяет Kepler (включая disabled back/forward chevrons) для точного совпадения.

## Constraints and non-goals

- Не менять id команд / IPC / окно `eden:settings` (сделано в предыдущей задаче).
- Не регрессить headless/test-mode.
- Один логический change; без попутных рефакторингов вне settings-обвязки.

## Acceptance Criteria

**AC1.** `@kosmos/visuals` экспортирует общую settings-обвязку: stylesheet (generic shell/header/section/row/typography) + компонент `SettingsContentHeader`.

**AC2.** Kepler `SettingsView` использует общий CSS + `SettingsContentHeader`; визуал Kepler settings не изменился (та же структура классов и правил).

**AC3.** `EdenSettingsView` рендерит ту же обвязку, что Kepler: `.settings`/`.settings-shell`/`.settings-content` + `SettingsContentHeader` + `SettingsSidebar`, без кастомных карточек-секций и больших h1/h2.

**AC4.** Tab-контент Eden (общие / корзина / vim) разложен по канону Kepler: капс-подписи секций + `SettingsList`/канон-строки, те же отступы.

**AC5.** Никакой выдуманной/самописной стилизации страницы в Eden: все page-shell стили приходят из общего `@kosmos/visuals` источника.

## Verification plan

- Typecheck: `bun run --cwd platform/desktop typecheck`; eden build/lint по возможности.
- Visuals/desktop unit (если затронуты): `bun test platform/desktop/electron/` (не должно регрессить).
- Headed (user): окно настроек Eden визуально совпадает с окном настроек Kepler (shell, header, секции, строки); Kepler settings без регресса.
