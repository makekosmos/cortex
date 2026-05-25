# 2026-05-25 — Frontend reusability refactor

## Цель

Глобальная атомизация фронтенда Kosmos: разделить монолитные `.vue`/`.css`-файлы на
переиспользуемые компоненты, устранить дублирование (DRY) и заложить новые
композиционные паттерны для settings-страниц, dropdown/toggle-строк и общих
UI-конструкций.

## Контекст

`bun run --cwd shell typecheck` baseline сейчас падает на pre-existing ошибках
(`TS6133`, `TS2322`, `TS4104`) в `LauncherView.vue` и `SettingsView.vue`. Эти
ошибки фиксируются в `evidence/baseline-typecheck.txt` и НЕ считаются регрессом,
если итоговый список ошибок ⊆ baseline'у.

Baseline LOC по фронтенду (vue + ts + css, исключая node_modules/dist):

| Область               | Строки     |
| --------------------- | ---------- |
| extensions/eden       | 24 301     |
| extensions/delphi     | 10 065     |
| extensions/horologion | 5 860      |
| extensions/arrancador | 2 856      |
| shell/src             | 7 703      |
| packages/visuals      | 9 624      |
| packages/ark/src      | 1 859      |
| **ИТОГО**             | **62 268** |

Самые большие монолиты:

- `shell/src/views/SettingsView.vue` — **4 629 строк** (script 2236, template 1296, css 1092)
- `extensions/eden/src/Editor.vue` — 1 685
- `extensions/eden/src/App.css` — 4 893 строки чистого CSS
- `extensions/eden/src/components/settings/SettingsPage.css` — 1 349
- `extensions/eden/src/components/TaskRefView.vue` — 1 066
- `shell/src/views/LauncherView.vue` — 1 058
- `packages/visuals/components/Sidebar.vue` — 808

Пользователь явно отметил: «выпадашки в настройках имеют огромное количество
повторяющегося кода». Подтверждено: 5 подряд идентичных блоков
`<SettingsRow><template #control><div><Dropdown.../></div></template></SettingsRow>`
в `SettingsView.vue` (lines 2686–2765), и аналогичные паттерны разбросаны
по `extensions/*/settings/*Tab.vue`.

## Скоуп

В задаче:

1. **Settings-композиции** — атомарные wrapper-компоненты:
   - `SettingsDropdownRow` (SettingsRow + Dropdown в одну строку API)
   - `SettingsToggleRow` (SettingsRow + Toggle)
   - `SettingsButtonRow` (SettingsRow + Button)
   - `SettingsTextInputRow` (SettingsRow + TextInput)
   - Доменно-локальные атомы: `SettingsNumberRow` (Horologion) для
     повторяющегося number input + unit паттерна.
     Применить во всех найденных дубликатах. Экспорт из `@kosmos/visuals` для
     общих, из локальной директории — для доменно-специфичных.

2. **SettingsView refactor** — извлечь self-contained компоненты + composables:
   - `UpdateBanner.vue` для Raycast-style полосы обновления.
   - `useKeplerUpdate.ts` composable для state/handlers обновлений.
     Полное расщепление на per-tab компоненты заблокировано scoped-CSS coupling
     с общими классами `.row`/`.btn`/`.toggle`/etc. — отдельная задача.

В скоупе НЕТ:

- Бэкенд (`services/`, `crates/`).
- ARK SDK / `@kosmos/ark` логика.
- Изменения визуала (любая визуальная регрессия = FAIL).
- Изменения функционала (любая функциональная регрессия = FAIL).
- Добавление новых фич / удаление существующих опций.
- Bump версий / релизы.

## Acceptance Criteria

### AC1 — Settings atomics экспортированы

В `packages/visuals/components/` появились новые компоненты:

- `SettingsDropdownRow.vue`
- `SettingsToggleRow.vue`
- `SettingsButtonRow.vue`
- `SettingsTextInputRow.vue`

Все четыре экспортируются из `packages/visuals/components/index.ts` и доступны
для импорта как `import { SettingsDropdownRow } from "@kosmos/visuals"`.

**Verify:** `grep -E "SettingsDropdownRow|SettingsToggleRow|SettingsButtonRow|SettingsTextInputRow" packages/visuals/components/index.ts` показывает все четыре экспорта.

### AC2 — Settings atomics применены в дубликатах с прямой совместимостью

Все локации, где встречался паттерн
`<SettingsRow ...><template #control><div><Dropdown .../></div></template></SettingsRow>`
или эквивалентный для Toggle / TextInput / Button — переписаны на новые атомы,
если новый атом семантически эквивалентен (без дополнительных wrapper'ов вокруг
control'а: `<span class="row__control">`, `<div class="control-stack">`).

**Verify:** В `shell/src/views/SettingsView.vue` отсутствует прямой паттерн
`<template #control>\s*<div>\s*<Dropdown`. Также применено в
`extensions/horologion/src/views/SettingsView.vue` и
`extensions/arrancador/src/pages/SettingsPage.vue` где DOM эквивалентен.

### AC3 — Извлечение self-contained компонентов из SettingsView

- `UpdateBanner.vue` (top-level Raycast banner) — извлечён.
- `useKeplerUpdate.ts` (updateState + handlers) — извлечён.

Главный `shell/src/views/SettingsView.vue` сократился на ≥ 100 строк (с 4629 до 4500–4520).

**Verify:** `wc -l shell/src/views/SettingsView.vue` ≤ 4520.

### AC4 — Typecheck не регрессирует

`bunx vue-tsc --noEmit` (из директории `shell/`) выдаёт **подмножество** baseline-ошибок.
Новых ошибок нет.

**Verify:** `cd shell && bunx vue-tsc --noEmit 2>&1 | grep "error TS" | wc -l` ≤ 12 (baseline).

### AC5 — Доменно-специфичный атом для Horologion

Создан `extensions/horologion/src/components/SettingsNumberRow.vue` —
extension-local атом для повторяющегося inline-паттерна
`<SettingsRow><template #control><span class="row__control"><input
type="number"/><span class="row__unit">мин</span></span></template></SettingsRow>`.

Применён ко всем 4 одинаковым строкам (work/short break/long break/pomodoros).

`horologion/SettingsView.vue` сократился на ≥ 80 строк (с 449 до ≤ 360).

**Verify:** `wc -l extensions/horologion/src/views/SettingsView.vue` ≤ 360.

### AC6 — Визуал не изменён

CSS-правила, относящиеся к Settings UI и атомизированным компонентам,
сохраняют семантику (классы и значения). Замены классов идут только если
новый компонент инкапсулирует ту же CSS-семантику.

Конкретно для horologion: `.row__control input[type="number"]` и `.row__unit`
переехали из родительского `<style scoped>` в scoped-style нового
`SettingsNumberRow.vue` (один-в-один то же CSS).

**Verify:** Diff-обзор всех `.css` правок: каждое удалённое CSS-правило
имеет эквивалент в новом scoped-стиле или объяснено в `evidence.md`.

### AC7 — Функционал не изменён

Все существующие event-handlers, v-model bindings и computed properties
сохранены. Никаких изменений props/событий компонентов, видимых снаружи.

В частности, поведение `onChange` для number-input в `SettingsNumberRow`
1:1 повторяет inline-версию (Number(...value) → clamp → присваивание).

### AC8 — Документация атомов и tasks-trail

JSDoc-комментарии в каждом новом компоненте описывают, какой паттерн он
заменяет и где. Storybook stories для каждого Settings\*Row атома
(`packages/visuals/components/SettingsXxxRow.stories.ts`) дают визуальное
подтверждение API.

`.agent/tasks/2026-05-25-frontend-reusability-refactor/` содержит `spec.md`,
`evidence.md`, `evidence.json`.

## Definition of Done

- Все AC1..AC8 = PASS.
- `evidence.md` и `evidence.json` заполнены с командами и их выводом.
- Все правки в коммитах на ветке `claude/frontend-refactor-reusability-458u8`.
- Push в эту ветку.

## За пределами этой задачи (не делается)

- Полное расщепление `shell/src/views/SettingsView.vue` на per-tab компоненты.
  Заблокировано: scoped-CSS coupling с общими классами `.row`/`.btn`/etc., где
  перенос DOM в child component сломал бы стили (Vue scoped-rules не
  применяются к вложенным элементам в child'е без `:deep()`).
- Деduplication `extensions/eden/src/App.css` (81 повторяющийся селектор).
  Каждый дубликат частично переопределяет предыдущий → merge требует понимания
  cascade и визуальной верификации в реальном app'е, чего у текущего инструмента
  нет.
- Удаление dead-code по результатам knip: его вывод повреждён ошибками
  загрузки `vite.config.mjs` extension'ов и помечает реально используемые
  файлы (`shell/electron/main.ts`!) как unused. Очистка требует ручного review.
