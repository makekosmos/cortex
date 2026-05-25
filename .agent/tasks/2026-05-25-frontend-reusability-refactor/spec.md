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

Baseline LOC по фронтенду (vue + ts + css, исключая node_modules/dist/.stories/.test):

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
   - `SettingsTextRow` (SettingsRow + TextInput)
     Применить во всех найденных дубликатах. Экспорт из `@kosmos/visuals`.

2. **SettingsView.vue split** — разбить на per-tab компоненты в
   `shell/src/views/settings/tabs/*.vue`. CSS для каждой вкладки уходит
   в scoped-style соответствующего таб-компонента. Главный
   `SettingsView.vue` остаётся orchestrator'ом ≤ 600 строк.

3. **Eden settings tabs** — где обнаружены аналогичные SettingsRow+control
   дубликаты, применить новые атомы из visuals.

4. **CSS-атомизация Eden App.css / Editor.css** — выделить
   общие токены/правила, которые уже дублируются между Editor и App.

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

Все три экспортируются из `packages/visuals/components/index.ts` и доступны
для импорта как `import { SettingsDropdownRow } from "@kosmos/visuals"`.

**Verify:** `grep -E "SettingsDropdownRow|SettingsToggleRow|SettingsButtonRow" packages/visuals/components/index.ts` показывает все три экспорта.

### AC2 — Settings atomics применены везде, где есть дубликат

Все локации, где встречался паттерн
`<SettingsRow ...><template #control><div><Dropdown .../></div></template></SettingsRow>`
(и аналогичный для Toggle) — переписаны на новые атомы.

**Verify:** В `shell/src/views/` и `extensions/*/src/` отсутствует прямой
паттерн `<template #control>\s*<div>\s*<Dropdown` (regex search возвращает 0
совпадений или ≤ 1, если оставлен по делу с custom-обвязкой).

### AC3 — `shell/src/views/SettingsView.vue` ≤ 600 строк

Главный файл расщеплён на per-tab компоненты:
`shell/src/views/settings/tabs/{General,About,Debug,Security,Secrets,Dictation,AppCommands,FileSearch,Extensions,Focus,Export}Tab.vue`.

Каждый tab-файл содержит свой `<template>`, `<script setup>` и scoped `<style>`.

**Verify:** `wc -l shell/src/views/SettingsView.vue` ≤ 600.

### AC4 — Typecheck не регрессирует

`bun run --cwd shell typecheck` (или `vue-tsc --noEmit`) выдаёт **подмножество**
baseline-ошибок. Новых ошибок нет.

**Verify:** `bunx --bun vue-tsc --noEmit -p shell/tsconfig.json` сравнивается с
`.agent/tasks/2026-05-25-frontend-reusability-refactor/evidence/baseline-typecheck.txt`.

### AC5 — Frontend LOC снижается ≥ 5%

Итоговый total LOC по списку директорий из контекста должен быть
**≤ 59 154** (=62 268 − 5%). Цель — реальное сокращение за счёт устранения
дублирования.

**Verify:** Script считает LOC ещё раз и сравнивает с baseline.

### AC6 — Визуал не изменён

CSS-правила, относящиеся к Settings UI и атомизированным компонентам,
сохраняют семантику (классы и значения). Замены классов идут только если
новый компонент инкапсулирует ту же CSS-семантику.

**Verify:** Diff-обзор всех `.css` правок: каждое удалённое CSS-правило
имеет эквивалент в новом scoped-стиле или объяснено в `evidence.md`.

### AC7 — Функционал не изменён

Все существующие event-handlers, v-model bindings и computed properties
сохранены. Никаких изменений props/события компонентов, видимых снаружи.

**Verify:** `grep -c "onUpdate\|@click\|@update:modelValue" shell/src/views/SettingsView.vue` (+ extracted tab файлы) ≥ baseline-count.

### AC8 — Документация обновлена

Один-два абзаца про новые атомы добавлены в `packages/visuals/components/index.ts`
или соответствующий `.md` (если есть). Если есть `docs-site/concepts/ui-atomics.md`,
он обновлён; если нет — новый файл НЕ создаётся (по правилу «новые .md без запроса»).

**Verify:** Изменения в `packages/visuals/` присутствуют в diff (JSDoc-комментарии
к новым компонентам).

## Definition of Done

- Все AC1..AC8 = PASS.
- `evidence.md` и `evidence.json` заполнены с командами и их выводом.
- Все правки в одном смысловом коммите на ветке `claude/frontend-refactor-reusability-458u8`.
- Push в эту ветку.
