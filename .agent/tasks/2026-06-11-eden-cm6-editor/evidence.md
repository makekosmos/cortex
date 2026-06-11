# Evidence — Eden CM6 editor, Vim mode and shared sidebar

Дата верификации: 2026-06-11. Scope расширен пользователем: CM6/Zennotes-like typing UX,
Vim mode/settings и sidebar через `@kosmos/visuals`.

## Delegation

- `Ohm` (`explorer`, `gpt-5.3-codex-spark`) — первая read-only команда по проверкам; упал на compact/max-output, результат не принят.
- `Maxwell` (`explorer`, `gpt-5.3-codex-spark`) — коротко подтвердил команды проверки и sandbox/Bun EACCES nuance.
- `Kuhn` (`explorer`, `gpt-5.3-codex-spark`) — нашёл e2e helpers/visual route для Eden и минимальный `window.kepler` surface.
- `Carver` (`explorer`, `gpt-5.3-codex-spark`) — разобрал Playwright configs: `platform/desktop --grep eden` = 0 tests, root config содержит Eden e2e.
- `Feynman` (`worker`, `gpt-5.4-mini`) — тестовый slice Vim settings/preferences/motions.
- `Poincare` (`worker`, `gpt-5.4-mini`) — `CmEditor.spec.ts` Vim smoke tests; brittle assertion был доработан parent'ом.
- `Popper` (`worker`, `gpt-5.4-mini`) — bounded cleanup `CmEditor.vue` unmount save/teardown.
- `Bohr` (`worker`, `gpt-5.4-mini`) — fixed false→true Vim reconfigure stale fat-cursor layer; parent tightened final selector.
- `Linnaeus` (`explorer`, `gpt-5.3-codex-spark`) — read-only audit of canonical `@kosmos/visuals` settings sidebar usage.
- `Hypatia` (`worker`, `gpt-5.4-mini`) — sidebar/settings test slice delegated; not blocking final implementation.

## Acceptance Criteria

### AC1 — `bun run test` — PASS

Команда: `bun run test` из `products/eden` (unsandboxed из-за Bun/package.json EACCES в sandbox).

- unit: `34 pass`, `0 fail`, `59 expect()`, `4 files`.
- browser: `10 passed`, `39 tests passed`.
- Включает `cmGate`, PM↔Markdown conversion, `CmEditor`, Vim settings, preferences и motions.

### AC2 — editor gate — PASS

- `shouldUseCmEditor` покрыт unit-тестами: pref off, safe doc, `taskRef`, `wikilink`, unknown node/mark, invalid JSON.
- `App.vue` монтирует `CmEditor` только через `shouldUseCmEditor(preferences.state.cmEditorEnabled, currentEntry.content_json)`.
- Default `cmEditorEnabled=false`, TipTap path остаётся fallback/default.

### AC3 — live preview and cursor UX — PASS

- `CmEditor.spec.ts` проверяет Zennotes-like `.cm-fat-cursor` layer for default CM mode.
- `CmEditor.spec.ts` проверяет `- [ ]` live checkbox на неактивной строке и click → `[x]` → `onSave`.
- Production visual screenshot: `.tmp/visual/2026-06-11-eden-cm6-editor/eden-notes-cm-vim-desktop.png`.

### AC4 — save pipeline — PASS

- `CmEditor.spec.ts` проверяет autosave через existing `onSave(entry)` с валидным PM JSON.
- Runtime path сохраняет старый ARK contract: markdown только в editor layer, storage остаётся `note_obj.contentJson` PM JSON.
- Unmount path запускает final save и сразу teardown'ит CodeMirror/converter, чтобы зависший `onSave` не держал view.

### AC5 — build/guards/colors — PASS

- `bunx vite build --config platform/desktop/vite.extensions.config.mjs --mode eden` — PASS.
- `bun run ark:guard:writes` — PASS.
- `bun run ark:smoke` — PASS после снятия lock с `target/debug/ark-core-rpc.exe` и прогрева Cargo cache.
- Grep по новым/затронутым `products/eden/src/editor-cm`, `VimSettings.vue`, `GeneralSettings.vue`, `EdenSidebar.vue`: нет `hex/rgb/rgba` literals.

### AC6 — visual verify — PASS

Visual script: `.tmp/visual/2026-06-11-eden-cm6-editor/visual-check.mjs`.

Screenshots inspected:

- `.tmp/visual/2026-06-11-eden-cm6-editor/eden-notes-cm-vim-desktop.png`
- `.tmp/visual/2026-06-11-eden-cm6-editor/eden-settings-vim-desktop.png`
- `.tmp/visual/2026-06-11-eden-cm6-editor/eden-settings-vim-compact.png`

Script result: `overflowCount: 0`. Checked visually: shared sidebar renders, Vim settings page
is readable on desktop/compact, CM editor opens from production bundle with Vim enabled and no Eden
custom `.cm-fat-cursorLayer` left over.

Note: `platform/desktop dev:extensions:only eden` currently fails because product-local Vite config
cannot resolve `vite` from `products/eden`; visual verify used production `dist` + narrow
`window.kepler` mock instead.

### AC7 — default path and existing tests — PASS

- `cmEditorEnabled=false` and `vimModeEnabled=false` by default.
- Full Eden test suite green after changes.
- TipTap fallback remains lazy async component and remains default for unsupported docs.
- Root Playwright Eden smoke: `bunx playwright test --config playwright.config.ts tests/e2e/eden.spec.ts -g "shim installed" --reporter=line --workers=1 --max-failures=1` — PASS (`1 passed`, real shell+Eden+ARK save/list/delete round-trip).
- Note: `cd platform/desktop; bunx playwright test --config playwright.config.ts --grep eden --list` returns `Total: 0 tests`; that config has no Eden tests and was an invalid verification target for this feature.
- Shell Playwright smoke after transient launch cleanup: `cd platform/desktop; bunx playwright test --config playwright.config.ts --reporter=line --workers=1 --max-failures=1` — PASS (`12 passed`).

### AC8 — Vim mode — PASS

- `@replit/codemirror-vim` added and wired through a CodeMirror `Compartment`.
- `vimModeEnabled` toggles Vim without changing bindings.
- Eden ex commands registered: `:w`, `:q`, `:wq`, `:zen`, `:zenmode`.
- Tests: `:w` calls `onSave`; false→true reconfigure removes Eden custom cursor layer while allowing Vim-owned cursor.

### AC9 — Vim settings — PASS

- Sidebar settings has `settings-nav-vim`.
- `VimSettings.vue` shows toggle and readonly grouped motions/actions (`VIM_MOTION_GROUPS`) on Russian UI.
- Tests cover settings page render, disabled note when CM6 is off, preference isolation, and motions list contract.

### AC10 — shared settings sidebar — PASS

- `EdenSidebar.vue` now uses `SettingsSidebar` + `SettingsSidebarButton` from `@kosmos/visuals`, matching shell Settings/Dashboard sidebar visual language.
- Eden Notes sidebar and Eden Settings sidebar are the same shared settings-sidebar surface.
- `SettingsSidebarButton` gained compatible `testId` and `contextmenu` support so Eden keeps existing test hooks and entry context menu without local button forks.
- Eden layout clamps persisted sidebar width to the shared settings sidebar width (`228px`) instead of old resizable widget widths.
- Removed legacy/nonexistent `hidden-width` prop, dead `initialConfig`/`configChange`/`allEntries` sidebar contract, and unused local `ObjectTypesSidebar.vue`.
- Replaced local SVG icon module with lucide icons.
- Removed stale `edenSidebarIcons.ts`.
- `EdenSidebar.spec.ts` covers notes actions, settings navigation, Vim tab entry, object-types navigation and back action via stable test ids.
- Final visual verify rerun after cleanup: notes/settings/compact screenshots refreshed, `overflowCount: 0`.

## Residual Manual

Real Kepler shell manual feel test remains in `docs-site/agents/manual-tests-pending.md`: typing feel,
persistence after real ARK reload, and real interactive Vim motions in Shell window.
