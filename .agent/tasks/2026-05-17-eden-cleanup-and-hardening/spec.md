# 2026-05-17 eden-cleanup-and-hardening

## Context

Phase 6.0 (`.agent/tasks/2026-05-17-eden-extension/`) поставил Eden как Vue extension с базовым note CRUD через ARK. Несколько фич оказались deferred: Hevy stub'нут, code lint/format stub'нут, trash UI пустой, vault picker удалён.

Решение пользователя 2026-05-17:

- **Hevy полностью отказываемся** — будет заменён Olympia позже (отдельное приложение).
- **Code tools удаляем полностью** — даже UI. Возможно вернётся позже.
- **Trash UI — делаем** через ARK soft-delete.
- **`apps/eden/ts/` — удаляем** (standalone Eden ушёл, дублирование больше не нужно).
- **Бандл — codesplit** (1.7MB → меньше).
- **Документация и roadmap** — обновить, добавить entry про per-type export через Kepler shell.

Этот proof loop — Phase 6.0.A: hardening + cleanup до состояния «extension Eden — единственный Eden».

## Scope

В задаче:

- Удалить Hevy полностью из `extensions/eden/`:
  - shim'е (`hevy*` stubs)
  - `vite-env.d.ts` (hevy типы)
  - `src/lib/hevy.ts` (если есть)
  - `src/components/settings/ConnectedAppsSettings.vue` (вся Hevy секция; settings rail убрать пункт)
- Удалить code-tools UI полностью из `extensions/eden/`:
  - shim (`lintCodeBlock`, `formatCodeBlock`, `getCodeToolsSettings`, `updateCodeToolsSettings`)
  - vite-env.d.ts (code-tools типы)
  - `src/lib/codeBlocks.ts` — только если он чисто про lint/format; если он также определяет `CodeBlockView`, сохранить TipTap extension и удалить только lint hookup
  - `Editor.vue` — убрать вызовы `lintCodeBlock` / `formatCodeBlock`, оставить CodeBlock TipTap extension работающим
  - settings UI (CodeToolsSettings панель) — убрать пункт из rail и сам panel
- Реализовать trash UI через ARK soft-delete:
  - `listTrashEntries()` в shim → `list_objects` где `deletedAt != null`
  - `restoreEntry(id)` → `upsert_object` с `deletedAt: null` (используя текущий объект + clearing deletedAt)
  - `permanentDeleteEntry(id)` → ARK `delete_object` (hard вариант если есть, иначе оставить permanent flag)
  - `getVaultStorageInfo` → подсчёт через `list_objects` (count active vs trash)
  - `purgeExpiredTrash` → no-op для extension (или TTL по `updatedAt`, опционально)
  - `TrashSettings.vue` интегрировать с реальным API — должна показывать корзинные объекты
- Codesplit `extensions/eden/`:
  - Lazy import Editor.vue в App.vue через `defineAsyncComponent` или `() => import("./Editor.vue")`
  - Aim: main bundle < 800KB
- Удалить `apps/eden/` директорию полностью (включая `apps/eden/ts`, `apps/eden/AGENTS.md`, `apps/eden/kotlin` если есть; `apps/eden/kotlin` — planned mobile Eden — пока удаляем, в roadmap зафиксируем)
- Зачистить ссылки на удалённый код:
  - `package.json` → убрать `apps/eden/ts` из workspaces
  - `Cargo.toml` → убрать `apps/eden/ts/heart` из workspace.members
  - `lefthook.yml` → убрать все hooks про `apps/eden/ts/**` (lint, format, typecheck, heart cargo test)
  - `scripts/check-ark-write-boundaries.mjs` → убрать `apps/eden/ts/main` из guard list
  - `scripts/ark-smoke.mjs` → убрать Eden migration smoke step
  - `scripts/fix-mojibake.mjs` → убрать `apps/eden/ts/main/store.ts` из списка
  - `scripts/sync-agents-docs.mjs` → убрать генерацию `apps/eden/AGENTS.md` и `apps/eden/ts/AGENTS.md`
  - `scripts/check-docs-freshness.mjs` → проверить, убрать ссылки на apps/eden пути
- Обновить документацию:
  - `STATUS.md` — добавить Phase 6.0 + 6.0.A в Сделано, обновить таблицу версий, убрать упоминания «Eden остаётся standalone»
  - `docs-site/apps/eden.md` — переписать как Vue extension: убрать про Electron main / Heart sidecar / preload API, добавить про shim + Kepler host integration
  - `docs-site/concepts/extension-host.md` — таблица «Phase 4 итог миграции» Eden: ❌ → ✅
  - `docs-site/agents/checklists.md` — Eden чек-лист переписать под extension
  - `docs-site/agents/forbidden.md` — убрать pre-existing Eden запреты которые больше не применимы (Hevy related, code tools related, vault picker)
  - `docs-site/concepts/architecture.md` + `docs-site/concepts/write-boundary.md` — обновить упоминания Eden как standalone
  - `docs-site/apps/kepler-roadmap.md` (или создать `docs-site/apps/kepler-roadmap.md` если нет) — добавить entry «Per-type data export (Phase 7+)»: notes → markdown файлы, runs → zip + GPX, и т.п. — единый export-pipeline в shell, типы-specific формат
  - `docs-site/reference/smoke-matrix.md` — убрать строки про Eden Rust Heart / Eden TS tests
- Прогнать `bun run docs:sync` → перегенерация `AGENTS.md`, `CLAUDE.md`, `docs-site/public/llms.txt`
- Прогнать `bun run docs:check` → подтвердить нет stale references

Не в задаче:

- Universal extension contract (отдельный Loop B)
- Eden e2e UI spec (Loop B)
- Реальный Eden brand icon (XS, оставлен placeholder; отдельная не-proof-loop правка)
- Per-type export pipeline сам по себе — только roadmap entry, реализация — Phase 7+
- Удаление `apps/eden/kotlin` mobile staging — если папка существует пустая/планируемая, убрать вместе с `apps/eden/`. Если нет — no-op.

## Acceptance Criteria

**AC1.** Hevy полностью убран из `extensions/eden/`:

- `grep -RE "hevy" extensions/eden/src/` — пусто (case-insensitive).
- `extensions/eden/src/lib/hevy.ts` отсутствует.
- В `vite-env.d.ts` нет `hevy*` методов в `Window.api`.
- В `ConnectedAppsSettings.vue` нет Hevy секции; если этот файл больше не нужен — удалить.

**AC2.** Code-tools UI убран из `extensions/eden/`:

- В shim нет `lintCodeBlock`, `formatCodeBlock`, `getCodeToolsSettings`, `updateCodeToolsSettings`.
- В `vite-env.d.ts` нет code-tools методов.
- `Editor.vue` не вызывает lint/format.
- CodeTools settings panel удалён из settings rail.
- TipTap CodeBlock extension сам по себе работает (синтакс highlight через lowlight остаётся).

**AC3.** Trash UI работает:

- `TrashSettings.vue` рендерит непустой список если в ARK есть объекты с `deletedAt != null`.
- Кнопка «Восстановить» обнуляет `deletedAt` (next `listEntries` показывает заметку в основном списке).
- Кнопка «Удалить навсегда» убирает объект из ARK (next `list_objects` его не возвращает).

**AC4.** Eden bundle codesplit:

- `extensions/eden/dist/assets/*-*.js` — main chunk < 800KB (gzip < 280KB).
- Editor chunk lazy-loaded (отдельный JS asset, появляется только при открытии заметки).

**AC5.** `apps/eden/` директория удалена:

- `Test-Path apps/eden` → False.
- `grep -RE "apps[/\\\\]eden" .` (исключая `.agent/`, `dist/`, `node_modules/`, `legacy/`) — пусто. Ссылки в активных конфигах вычищены.

**AC6.** Workspace + tooling cleanup:

- `package.json` workspaces не содержит `apps/eden/ts`.
- `Cargo.toml` workspace.members не содержит `apps/eden/ts/heart`.
- `lefthook.yml` не содержит hooks про `apps/eden/`.
- `scripts/check-ark-write-boundaries.mjs` actively-guarded list не содержит `apps/eden/ts/main`.
- `scripts/ark-smoke.mjs` не запускает Eden TS migration test.
- `scripts/sync-agents-docs.mjs` не генерирует `apps/eden/AGENTS.md` и `apps/eden/ts/AGENTS.md`.

**AC7.** Документация обновлена и перегенерирована:

- `docs-site/apps/eden.md` описывает extension-based Eden, не standalone.
- `STATUS.md` содержит Phase 6.0/6.0.A.
- `docs-site/apps/kepler-roadmap.md` (или эквивалент) содержит entry про per-type export.
- `bun run docs:sync` отрабатывает без ошибок, перегенерированные `AGENTS.md` / `CLAUDE.md` / `llms.txt` не имеют ссылок на `apps/eden`.
- `bun run docs:check` зелёный (нет stale references).

**AC8.** Build + typecheck зелёные:

- `bun install` clean (после изменения workspaces).
- `bun run --cwd shell build:extensions` — все 4 extensions собираются.
- `bun run --cwd shell typecheck` — clean.
- `bun run ark:guard:writes` — зелёный.
- `cargo build --workspace` — собирается без `apps/eden/ts/heart`.

## Verification commands

См. AC.

## Out of scope decisions

- `apps/eden/kotlin` Android планировался, но не реализован. Удаляется в этом proof loop'е (либо проверяется отсутствие — если папки нет, no-op).
- Eden brand icon в extension'е остаётся `strontium.png` placeholder. Отдельная косметическая задача.
- Heart Rust sidecar binary полностью уходит — `cargo build --workspace` его больше не собирает. Если кому-то нужен Heart для импорт/экспорт utility — снимок исходников в `legacy/eden-heart/` не делаем, git history достаточен.
- Per-type export (notes → markdown, runs → GPX+zip и т.п.) — Phase 7+ задача, в roadmap.
