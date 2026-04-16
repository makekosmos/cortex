# Инструкции для агента

## Обязательная проверка перед сдачей

- Перед отдачей задачи пользователю всегда проверяй, что приложение успешно собирается без ошибок через `npm run build`.
- Перед отдачей задачи пользователю всегда запускай e2e тесты через `npm run test:e2e` и убеждайся, что они проходят.
- Если менялась только типизация или локальная логика без UI, все равно прогоняй хотя бы `npm run lint` и `npx tsc --noEmit`, но финальный стандарт для сдачи остается `npm run build` + `npm run test:e2e`.

## Текущий стек проекта

- UI: `Electron + Vue 3 + TipTap`
- Сборка: `electron-vite`, `vite 7`
- Язык: `TypeScript`
- Локальное хранилище: `SQLite` через `better-sqlite3`
- Линтинг и форматирование: `oxlint`, `oxfmt`
- E2E: `Playwright`
- Поиск по содержимому: первый `eden-heart` на `Rust + Tantivy`

## Важные архитектурные факты

- Проект больше не использует `vite-plugin-electron`; текущий конфиг лежит в `electron.vite.config.ts`.
- Основная Electron-логика находится в `main/main.ts`, `main/preload.ts`, `main/store.ts`.
- Desktop shell строится через shared `DesktopChrome` и `DesktopContentSurface` из `@kepler/visuals`; это основной контракт для оконного chrome, sidebar и content surface.
- Native controls и safe-area поведение задаются через `BrowserWindow` chrome config и shared visual components, а не через ручные offsets в основном shell.
- Не возвращай в shell старые manual titlebar offsets вроде локальных `--titlebar-height` / `--titlebar-left-safe-area` костылей, если их можно выразить через shared chrome contract.
- Typed notes уже начаты:
  - есть `note_types`
  - у `entries` есть `type_id`, `header_layout`, `header_props_json`, `schema_version`
  - shared typed-note логика лежит в `src/lib/typedNotes.ts`
- Типы заметок управляются через отдельную страницу `src/NoteTypesScreen.tsx`.
- Тело заметки остается обычным editor-body, а верхушка рендерится отдельно.
- Сайдбар, диалоги и typed header уже вынесены в компоненты; не тащи эту логику обратно в монолитный `App`.

## Search / eden-heart

- `eden-heart` находится в `heart/`.
- Сейчас это первый Rust sidecar для поиска, вызываемый из Electron через `main/heart.ts`.
- Не возвращай проект к `ripgrep`; поиск уже должен развиваться через `eden-heart`.
- Следующий этап для поиска — не новый хак в JS, а инкрементальный индекс и расширение search contract в Rust.

## Storage / надежность

- В `main/store.ts` уже есть hardening для сохранения заметок и перемещения папок.
- Не ломай tree-aware path logic: для markdown-файлов нужно использовать единый путь заметки, а не старые плоские пути.
- Особенно аккуратно относись к операциям `save/move/delete`: здесь уже есть защита от потери заметок, не упрощай ее без необходимости.

## UI / стиль

- Это личное desktop-приложение, не generic web app.
- Держись системного desktop-ощущения, не тяни веб-шрифты без необходимости.
- Ориентир по визуалу: чистый native desktop shell, аккуратный chrome, без тяжелых эффектов ради эффекта.
- При изменении UI сохраняй ощущение нативного приложения и хорошую адаптивность на узких окнах.
- Если меняешь shell, сначала думай в терминах `DesktopChrome` / `DesktopContentSurface`, а не через ручные padding/margin offset hacks.

## Кодстайл

- Предпочитай более чистый и читаемый код вместо “умного” кода.
- Используй alias `@/` для импортов из `src`, если это не ломает читаемость.
- Перед сдачей обязательно прогоняй `oxlint`; не оставляй warnings.
- Если добавляешь новый слой логики между UI и backend, предпочитай отдельный фасад/сервис, а не прямые вызовы по всему приложению.

## E2E / dev UX

- Во время e2e окно Electron не должно воровать фокус.
- Если меняешь процесс запуска окна, не ломай background launch для тестов.
- E2E могут проверять не только UI, но и файлы на диске; не упрощай тесты так, чтобы они перестали ловить регрессии хранилища.
