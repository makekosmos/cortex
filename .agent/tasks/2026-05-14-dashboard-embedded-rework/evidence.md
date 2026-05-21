# Evidence — dashboard-embedded-rework

**Date**: 2026-05-14
**Status**: completed

## Commits

```
d0a36b3 docs: rewrite dashboard.md под embedded архитектуру
78366e5 feat(dashboard): space view с sidebar + object table
1546952 feat(dashboard): spaces preload API + listSpaces handler
95c1465 feat(dashboard): scaffold welcome view + dashboard window + tray entry
7c68f11 refactor: freeze extensions/dashboard → legacy/dashboard-extension
```

## Files

### New

- `shell/electron/dashboard-window.ts`
- `shell/src/views/DashboardRoot.vue`
- `shell/src/views/DashboardWelcomeView.vue`
- `shell/src/views/DashboardSpaceView.vue`
- `shell/src/dashboard/KosmosLogo.vue`
- `shell/src/dashboard/SpaceCard.vue`
- `shell/src/dashboard/SidebarItem.vue`
- `shell/src/dashboard/ObjectTable.vue`
- `shell/src/dashboard/store.ts`
- `shell/src/dashboard/types.ts`

### Modified

- `shell/electron/commands.ts` (static dashboard:open команда → openDashboardWindow)
- `shell/electron/main.ts` (tray «Dashboard» item; kepler:spaces:list + kepler:ark:request IPC handlers; listSpaces())
- `shell/electron/preload.ts` (window.kepler.spaces.list / window.kepler.ark.request bridges)
- `shell/shared/ipc-types.ts` (SpaceMeta + KeplerApi.spaces.list + KeplerApi.ark.request)
- `shell/src/main.ts` (hash → root view dispatch включая #/dashboard/\*)

### Moved to legacy/

- `extensions/dashboard/` → `legacy/dashboard-extension/`

### Docs

- `docs-site/apps/dashboard.md` (полный rewrite)
- `docs-site/apps/index.md` (Dashboard перенесён в секцию «Встроенные shell views»)
- `docs-site/apps/kepler.md` (Dashboard убран из bundled extensions list)
- `docs-site/apps/kepler-roadmap.md` (отметка о rewrite в Phase 4 секции)
- `docs-site/agents/forbidden.md`, `checklists.md`, `index.md`
- `docs-site/concepts/write-boundary.md`
- AGENTS.md / CLAUDE.md / apps/\*/AGENTS.md / public/llms.txt (auto-generated)

## Acceptance criteria

| AC                                                                        | Status | Evidence                                                                                                                                                   |
| ------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC1: Tray «Dashboard» → 1200×800 window                                   | PASS   | tray menu в `main.ts:347-359` (commit 95c1465); window size в `dashboard-window.ts:11-14`                                                                  |
| AC2: Welcome показывает logo / «Kosmos» / минимум 1 space card            | PASS   | DashboardWelcomeView renders KosmosLogo + brand + SpaceCard, listSpaces() добавляет selected-space.json fallback                                           |
| AC3: Click space card → space view                                        | PASS   | SpaceCard emits open, DashboardWelcomeView sets `window.location.hash = "#/dashboard/space/<id>"`, DashboardRoot watches hashchange                        |
| AC4: Sidebar показывает Всё / Настройки / object_types                    | PASS   | DashboardSpaceView calls loadObjectTypes() → list_object_types ARK op, renders SidebarItem'ы                                                               |
| AC5: Click sidebar → main pane таблица объектов                           | PASS   | selectType(id) calls loadObjects(id) → list_objects_by_type, ObjectTable renders rows                                                                      |
| AC6: Columns Значение/Тип/Добавлено/Данные X/Данные Y с реальными данными | PASS   | store.toRow() резолвит primary (title или contentJson string), dataX/dataY (contentJson+propsJson values, «—» fallback), createdAt formatted               |
| AC7: `build:js` PASS, typecheck PASS                                      | PASS   | tsc --noEmit clean; vite build → dist/index.html + DashboardRoot-_.js + DashboardRoot-_.css                                                                |
| AC8: Закрытие dashboard окна не закрывает Kepler shell                    | PASS   | dashboard-window.ts: dashboardWin.on("closed", () => { dashboardWin = null }); никаких app.quit() побочных эффектов; main launcher window живёт независимо |
| AC9: `docs:check` 0 stale                                                 | PASS   | `bun run docs:check` → «всё свежо, stale references не найдено»                                                                                            |
| AC10: extensions/dashboard frozen, launcher работает                      | PASS   | `git mv extensions/dashboard → legacy/dashboard-extension`; static `dashboard:open` command теперь exec'ит openDashboardWindow()                           |

## Guards

- `bun run ark:guard:writes` — PASS
- `bun run docs:sync` — PASS
- `bun run docs:check` — PASS
- `bun run --cwd shell typecheck` — PASS
- `bun run --cwd shell build:js` — PASS

## Appendix — пост-тестовые фиксы (2026-05-15)

Пользователь после первого приёмочного теста зарепортил 3 проблемы. Все
закрыты атомарными коммитами поверх стека:

### Commits

```
e56f19e feat(dashboard): listSpaces читает spaces.json registry (2 vs 4 dups fix)
ea55180 feat(dashboard): objectCount через node:sqlite read-only
853cb12 refactor(dashboard): use @kepler/visuals shared компоненты
```

### Files modified

- `shell/electron/main.ts` — `readSpacesRegistry`, `labelFromSpaceCode`,
  `countObjectsInSpaceDb`, `loadDatabaseSync`, async `listSpaces`.
- `shell/src/views/DashboardWelcomeView.vue` — wrap в `DesktopChrome` +
  `DesktopContentSurface` (scrollable).
- `shell/src/views/DashboardSpaceView.vue` — wrap в `DesktopChrome`
  - sidebar slot + `DesktopContentSurface` (padding=0).

### Acceptance criteria (appendix)

| AC                                                             | Status | Evidence                                                                                                                                                                                                                                           |
| -------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Appendix AC1: `listSpaces()` возвращает ровно 2 entries (не 4) | PASS   | `spaces.json` registry читается как source of truth, derived spaceId через `deriveSpaceIdFromCode(code)`. На текущей машине: Personal (`f028287f78de2d7e`) + MWVQ-YBRE-WTQK (`2b42c913678b1572`). Осиротевшие dirs `76a639…` и `main` игнорируются |
| Appendix AC2: objectCount показывает реальное число            | PASS   | `node:sqlite` DatabaseSync read-only + `SELECT COUNT(*) FROM objects WHERE deleted_at IS NULL`. Personal → 26 (verified через ad-hoc node script). MWVQ → 0 (DB exists без schema, «no such table» fallback)                                       |
| Appendix AC3: Welcome / Space используют DesktopChrome         | PASS   | `<DesktopChrome platform="windows">` обёрнут вокруг root в обоих views, sidebar через `#sidebar` slot в Space view, content через `<DesktopContentSurface>`                                                                                        |
| Appendix AC4: `bun run --cwd shell build:js` PASS              | PASS   | vite build clean, DashboardRoot-DH-JFkR6.js / DashboardRoot-DnZZlYo6.css emitted                                                                                                                                                                   |
| Appendix AC5: typecheck PASS                                   | PASS   | `bun run --cwd shell typecheck` clean после каждого коммита                                                                                                                                                                                        |
| Appendix AC6: Visual consistency                               | PASS   | DesktopChrome даёт ту же titlebar drag area + sidebar slot, что и у Delphi/Horologion extensions; единые tokens из @kepler/visuals                                                                                                                 |

### Open notes

- `node:sqlite` experimental warning печатается при первом импорте при
  открытии Dashboard'а. Подавлено dynamic import'ом — warning не сыпется
  при старте Kepler. Когда модуль стабилизируется в Node 24 LTS — можно
  убрать lazy import.
- Shared `Sidebar` из @kepler/visuals требует vue-router + проектные
  группы — overkill для flat type list. Custom SidebarItem оставлен.
- backend kepler-backend.exe держит активный space DB open в r/w —
  одновременное чтение через `node:sqlite` read-only возможно благодаря
  WAL mode SQLite.
