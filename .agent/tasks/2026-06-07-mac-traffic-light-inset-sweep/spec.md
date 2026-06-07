# 2026-06-07 mac-traffic-light-inset-sweep

## Context

На macOS extension-окна создаются с `titleBarStyle: "hidden"` +
`trafficLightPosition: { x: 16, y: 14 }` (`extension-host.ts` →
`mac-window.ts`). Каждое extension-окно получает `data-platform="mac"` на
`<html>` (`extension-preload.ts`), что активирует токен
`--kosmos-mac-traffic-light-left-safe-area: 80px` (+ `…-top-safe-area: 28px`)
из `@kosmos/visuals/theme/css-variables.css`.

eden и delphi уже учитывают этот inset (свой `platform-mac` / `:platform="mac"`).
Два extension'а — **не учитывают**, контент налезает на нативные traffic lights:

1. **arrancador** — кастомный `AppTitlebar` (`styles.css .arrancador-titlebar`)
   имеет safe-area только справа (под Windows-overlay кнопки), слева
   захардкожен `16px` → menu-кнопка + заголовок под traffic lights.
2. **akasha** — `<DesktopChrome>` **без `:platform`** → дефолт `"windows"` →
   `Titlebar.vue` применяет Windows-env() padding (0px на mac) вместо mac-токена.
   В reader-режиме back-кнопка под traffic lights. В library-режиме (overlay
   titlebar `left: 228px`) верх сайдбара (`book-library-sidebar`, `padding-top:
14px`) налезает на traffic lights по вертикали.

Плюс stale-ссылка: `main.ts` benchmark-список
`["horologion", "delphi", "arrancador", "eden"]` упоминает архивированный
horologion (`manifest.archived.json`, мигрирован в focus mode).

## Scope

В задаче:

- arrancador `styles.css`: левый padding `.arrancador-titlebar` →
  `max(16px, var(--kosmos-mac-traffic-light-left-safe-area, 0px))`. На Windows
  токен = 0px → остаётся 16px; на mac = 80px.
- akasha `AkashaApp.vue`: `usePlatform()` из `@kosmos/visuals`, прокинуть
  `:platform="platform"` в `<DesktopChrome>` (фикс reader-режима через
  `Titlebar.vue .kosmos-titlebar--mac`).
- akasha `BookLibrarySidebar.vue`: `padding-top` → учесть
  `var(--kosmos-mac-traffic-light-top-safe-area, 0px)` (фикс library-режима).
- `main.ts`: убрать `"horologion"` из benchmark-списка `KEPLER_BENCHMARK_OPEN_ALL`.

Не в задаче:

- Полное удаление `incubator/horologion/` + 15 e2e-spec'ов (`horologion-*.spec.ts`)
  — отдельный proof loop с разбором тестовых зависимостей. horologion в коде
  уже исключён из всех discovery-путей (нет `manifest.json`/`package.json`);
  его появление в живом UI — артефакт stale dev-инстанса (restart чинит).
- Миграция delphi/eden bespoke mac-детекции на общий `usePlatform` (работает,
  не баг).
- Top-safe-area для контента extension'ов вне титлбара/сайдбара (контент уже
  под титлбаром, не под traffic lights).

## Acceptance Criteria

AC1. arrancador: на mac titlebar leading (menu + "Arrancador") начинается
правее traffic lights; на Windows — без регрессии (16px). Проверка: build +
visual (mac) / grep токена в padding.

AC2. akasha reader-режим: back-кнопка правее traffic lights (`:platform="mac"`
прокинут → `.kosmos-titlebar--mac` padding-left). Проверка: build + visual.

AC3. akasha library-режим: верх сайдбара (search) ниже traffic lights
(top-safe-area). Проверка: build + visual.

AC4. `main.ts` benchmark-список не содержит `"horologion"`.

AC5. `bun run --cwd platform/desktop typecheck` зелёный.
`bun run build:extensions` (akasha, arrancador) собирается без ошибок.

## Verification commands

- `grep -n "mac-traffic-light-left-safe-area" incubator/arrancador/src/styles.css` — AC1.
- `grep -n "usePlatform\|:platform" incubator/akasha/src/components/AkashaApp.vue` — AC2.
- `grep -n "mac-traffic-light-top-safe-area" incubator/akasha/src/components/BookLibrarySidebar.vue` — AC3.
- `grep -n "horologion" platform/desktop/electron/main.ts` пусто — AC4.
- `bun run --cwd platform/desktop typecheck` — AC5.
- Visual (mac, dev running): открыть Arrancador / Akasha (library + reader) —
  screenshot под `.tmp/`, traffic lights не перекрывают контент.

## Out of scope decisions

- horologion source eradication остаётся открытым долгом — выношу в отчёт,
  не в этот commit.
