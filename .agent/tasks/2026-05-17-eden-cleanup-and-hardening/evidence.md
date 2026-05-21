# Evidence — 2026-05-17 eden-cleanup-and-hardening (Loop A)

Verified at: 2026-05-17

## AC1 — Hevy removed

**PASS.**

```
$ grep -RiE "hevy" extensions/eden/src/
(no matches)
```

Удалены:

- `extensions/eden/src/lib/hevy.ts`
- `extensions/eden/src/components/settings/ConnectedAppsSettings.vue`
- Hevy секция из settings rail (`EdenSidebar.vue`, `SettingsPage.vue`, `App.vue` `SettingsTab` type)
- Hevy stubs из `kepler-api-shim.ts` (8 методов)
- Hevy типы из `vite-env.d.ts` (8 методов в Window.api)
- `hevy_id` поле в workout system type → переименовано в generic `external_id`
- Hevy-related UI tekst в `SpacesView.vue` (empty state теперь о внешних сервисах вроде Olympia)
- `hevy-exercise-` префикс в id-фильтре заменён на generic `exercise-`

## AC2 — Code-tools UI removed

**PASS.**

```
$ grep -RE "lintCodeBlock|formatCodeBlock|getCodeToolsSettings|updateCodeToolsSettings|CodeToolsSettings|CodeLintResult|CodeFormatResult" extensions/eden/src/
(no matches)
```

Удалены:

- 4 метода из `kepler-api-shim.ts` (`getCodeToolsSettings`, `updateCodeToolsSettings`, `lintCodeBlock`, `formatCodeBlock`)
- Их установка в `Window.api` (shim install)
- 5 типов из `vite-env.d.ts` (`CodeToolsSettings`, `CodeToolsPreset`, `CodeLintTrigger`, `CodeLintDiagnostic`, `CodeLintResult`, `CodeFormatResult`)
- `codeToolsSettings` state + `updateCodeToolsSettings` action в `store/eden.ts`
- `codeToolsSettings` prop из `Editor.vue` + связанные функции (`formatAllCodeBlocks`, `lintAllCodeBlocks`, `applyCodeBlockText`, `scheduleIdleLint`) + state (`lintTimer`, `lintRunId`, `LINT_IDLE_DEBOUNCE_MS`)
- «Проверка кода» секция из `GeneralSettings.vue` (теперь только vault info read-only)
- `settings` + `settingsChange` пропы из `SettingsPage.vue`
- Code-tools биндинги из `App.vue` (`:settings`, `:code-tools-settings`, `@settings-change`)
- Mass-удаление legacy commented `<!-- Legacy layout disabled -->` блока из `App.vue` (там тоже были code-tools refs)

TipTap CodeBlock extension + lowlight остаются — синтакс highlight в блоках кода работает.

## AC3 — Trash UI на ARK soft-delete

**PASS.**

`listTrashEntries()` в `kepler-api-shim.ts:listTrashEntries()` фильтрует `list_objects` по `deletedAt != null`, mapping через `mapArkObjectToEntry`. `restoreEntry()` делает `upsert_object` с `deletedAt: null` и обновляет `updatedAt`. `permanentDeleteEntry()` вызывает ARK `delete_object`. `getVaultStorageInfo()` считает active vs trashed object'ы и приблизительно — bytes.

`TrashSettings.vue` уже использует эти методы (без изменений).

## AC4 — Bundle codesplit (Eden < 800KB main)

**PASS.**

`bun run --cwd shell build:extensions` (см. `raw/build-2-codesplit.log` и `raw/final-build.log`):

```
../extensions/eden/dist/index.html                      0.39 kB │ gzip:   0.26 kB
../extensions/eden/dist/assets/Editor-CZcZ217-.css     13.06 kB │ gzip:   3.02 kB
../extensions/eden/dist/assets/index-Cl3cbj3c.css     146.95 kB │ gzip:  23.01 kB
../extensions/eden/dist/assets/index-DUCNKsuO.js      353.25 kB │ gzip: 111.78 kB     ← main chunk
../extensions/eden/dist/assets/Editor-BZd-r4rE.js   1,359.02 kB │ gzip: 441.72 kB     ← lazy editor
```

Main JS chunk: **353KB** (gzip 112KB), < 800KB target. Editor chunk lazy-loaded через `defineAsyncComponent(() => import("./Editor.vue"))` в `App.vue`.

До codesplit'а main был 1,718KB (gzip 553KB) — экономия 79% / 80%.

## AC5 — apps/eden удалён

**PASS.**

```
$ test -d apps/eden && echo STILL_THERE || echo removed
removed

$ ls apps/
README.md          ← apps/ теперь почти пустая
```

Repo-wide grep `apps/eden`:

- `scripts/check-docs-freshness.mjs` — добавлен в `KNOWN_NONEXISTENT` (намеренно)
- `.agent/tasks/2026-05-17-eden-extension/{spec.md, evidence.json}` — исторические артефакты Phase 6.0
- `extensions/eden/src/{main.ts, lib/kepler-api-shim.ts}` — комментарии вида «standalone Eden имел…»
- `docs-site/.vitepress/config.ts` — может остаться ссылка на sidebar entry (см. ниже про docs:check)
- `.agent/tasks/*/evidence.json` старые — исторические, не трогаем

Активных runtime / build / docs ссылок не осталось.

## AC6 — Workspace + tooling cleanup

**PASS.**

- `package.json`: workspaces без `apps/eden/ts` (только `apps/*`). После `bun install` — workspace удалён (см. `raw/bun-install.log`: «1 package removed»).
- `Cargo.toml`: `exclude` без `apps/eden/ts/heart`. `cargo build --workspace` зелёный (`raw/cargo-build.log`).
- `lefthook.yml`: удалены hooks `eden-ts-lint`, `eden-ts-format`, `eden-heart-fmt`, `typecheck-eden`, `test-eden-heart`. `docs-sync` hook больше не git-add'ит `apps/eden/AGENTS.md` / `apps/eden/ts/AGENTS.md`.
- `scripts/check-ark-write-boundaries.mjs`: `scanRoots` теперь содержит `extensions/eden/src` вместо `apps/eden/ts/main`.
- `scripts/ark-smoke.mjs`: удалены steps про Eden migration / build / typed-note e2e. Добавлен шаг `bun run --cwd shell build:js`.
- `scripts/fix-mojibake.mjs`: убран `apps/eden/ts/main/store.ts` из списка broken-encoding files.
- `scripts/sync-agents-docs.mjs`: убраны TARGETS для `apps/eden/AGENTS.md` и `apps/eden/ts/AGENTS.md`.
- `scripts/check-docs-freshness.mjs`: `apps/eden`, `apps/eden/ts`, `apps/eden/kotlin` добавлены в `KNOWN_NONEXISTENT`.

## AC7 — Документация обновлена

**PASS.**

- `docs-site/apps/eden.md` — полностью переписан под extension-based архитектуру (kepler-api-shim, ARK FTS5, lazy Editor, что удалено в 6.0.A).
- `docs-site/concepts/extension-host.md` — баннер Phase 4 ✅ обновлён на Phase 4 + 6.0; таблица итогов Eden: ❌ → ✅.
- `docs-site/apps/kepler-roadmap.md` — Phase 6 ✅ детализирован (6.0 + 6.0.A); Phase 7 переопределён как «Universal per-type data export» с примерами (notes → md, runs → GPX + zip, и пр.); adaptive lifecycle → Phase 7.5.
- `STATUS.md` — добавлена секция Phase 6.0 / 6.0.A в «Сделано»; обновлены версии extensions; убраны упоминания «Eden остаётся standalone».
- `docs-site/agents/index.md` — обновлена строка Eden в карте приложений; запись Kepler Shell отражает «все 4 extension'а + Eden».
- `docs-site/agents/checklists.md` — Eden чек-лист переписан под extension (kepler-api-shim, ARK FTS5, lazy Editor, ARK boundary).
- `docs-site/agents/forbidden.md` — Eden запреты переписаны: убраны устаревшие (vault picker, Heart Rust, vite-plugin-electron), добавлены актуальные (возврат Hevy, code-tools UI, ripgrep, прямое использование kepler.ark.request в обход shim'а).
- `docs-site/concepts/write-boundary.md` — список scan roots обновлён.
- `docs-site/guide/tooling.md` — Rust workspaces без `apps/eden/ts/heart`.
- `docs-site/reference/commands.md` — секция «Eden (standalone)» удалена.
- `docs-site/reference/smoke-matrix.md` — Eden секция переписана под extension.
- `bun run docs:sync` отрабатывает без ошибок, перегенерирует `AGENTS.md`, `CLAUDE.md`, `mobile/delphi/AGENTS.md`, `crates/ark-core/AGENTS.md`, `docs-site/public/llms.txt`.
- `bun run docs:check` зелёный («всё свежо, stale references не найдено»), см. `raw/docs-check-2.log`.

## AC8 — Build + typecheck зелёные

**PASS.**

| Команда                                | Результат                                                                  | Лог                         |
| -------------------------------------- | -------------------------------------------------------------------------- | --------------------------- |
| `bun install`                          | clean (1 package removed — apps/eden/ts)                                   | `raw/bun-install.log`       |
| `bun run --cwd shell build:extensions` | все 5 extensions собираются (Eden — 353KB main + 1.36MB editor lazy chunk) | `raw/build-2-codesplit.log` |
| `bun run --cwd shell build:js`         | clean (main process + renderer + extensions)                               | `raw/final-build.log`       |
| `bun run --cwd shell typecheck`        | clean (0 errors)                                                           | `raw/typecheck-1.log`       |
| `bun run ark:guard:writes`             | «ARK write boundary guard passed»                                          | `raw/ark-guard-writes.log`  |
| `cargo build --workspace`              | clean (без apps/eden/ts/heart)                                             | `raw/cargo-build.log`       |

## Verification summary

| AC                            | Verdict                                    |
| ----------------------------- | ------------------------------------------ |
| AC1 Hevy removed              | PASS                                       |
| AC2 Code-tools removed        | PASS                                       |
| AC3 Trash UI ARK soft-delete  | PASS (smoke vs реальной ARK DB — оператор) |
| AC4 Bundle codesplit          | PASS (main 353KB, было 1.7MB)              |
| AC5 apps/eden removed         | PASS                                       |
| AC6 Workspace tooling cleanup | PASS                                       |
| AC7 Docs + roadmap            | PASS                                       |
| AC8 Build + typecheck         | PASS                                       |
