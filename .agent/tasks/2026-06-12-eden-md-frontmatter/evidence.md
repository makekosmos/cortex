# Evidence

Verified on 2026-06-12.

## AC1 — Pure Markdown frontmatter codec

Verdict: PASS.

Evidence:

- Added `products/eden/src/lib/markdownFrontmatter.ts`.
- The module exports pure `buildEntryMarkdownDocument` and `parseEntryMarkdownDocument`.
- The module has no `window`, `ipc`, filesystem, or ARK calls.

Commands:

- `rtk git diff --check` — PASS.
- `rtk proxy bunx vite build --config vite.config.mjs` from `products/eden/` — PASS.

## AC2 — Exported metadata and Obsidian body

Verdict: PASS.

Evidence:

- Export frontmatter includes `eden.id`, `eden.type`, `eden.schema_version`, `title`, `type`, object fields from `header_props_json`, and `links.related`.
- Body export uses the existing Eden Markdown converter from `editor-cm/mdConvert`.

Commands:

- `rtk proxy bunx vite build --config vite.config.mjs` from `products/eden/` — PASS.

## AC3 — Import parser to entry draft

Verdict: PASS.

Evidence:

- `parseEntryMarkdownDocument` returns `frontmatter`, `bodyMarkdown`, and `entryPatch`.
- `GeneralSettings.vue` converts `bodyMarkdown` through `mdConvert.markdownToJson`, normalizes typed header props with `normalizeHeaderProps`, and saves through `window.api.saveEntry`.
- Unknown top-level frontmatter fields are preserved as header props unless reserved or internal.

Commands:

- `rtk proxy bunx vite build --config vite.config.mjs` from `products/eden/` — PASS.

## AC4 — Manual Russian UI path with safe preload API

Verdict: PASS.

Evidence:

- Added a Russian "Обмен с Markdown" section to `GeneralSettings.vue`.
- Added `window.kepler.markdownFiles.open/save` through `extension-preload.ts`.
- Added narrow Electron IPC handlers in `extension-host.ts`: open reads only the dialog-selected Markdown file, save writes only after save dialog confirmation.

Commands:

- `rtk proxy bunx tsc --noEmit -p platform/desktop/tsconfig.json` — PASS.
- `rtk proxy bunx vite build --config vite.config.mjs` from `products/eden/` — PASS.

## AC5 — ARK write boundary

Verdict: PASS.

Evidence:

- Import saves through `window.api.saveEntry`.
- No direct SQL write path was added.

Commands:

- `rtk proxy node scripts/check-ark-write-boundaries.mjs` — PASS.

## AC6 — No live file sync / ARK remains canonical

Verdict: PASS.

Evidence:

- Implementation only adds manual import/export actions.
- No filesystem watcher, polling, conflict resolver, or direct file-backed source-of-truth was added.

Commands:

- Code review of changed files.

## AC7 — Relevant verification recorded

Verdict: PASS.

Commands:

- `rtk git diff --check` — PASS.
- `rtk proxy node scripts/check-ark-write-boundaries.mjs` — PASS.
- `rtk proxy bunx tsc --noEmit -p platform/desktop/tsconfig.json` — PASS.
- `rtk proxy bunx vite build --config vite.config.mjs` from `products/eden/` — PASS.

Notes:

- `rtk bun run ark:guard:writes` and `rtk bun run --cwd platform/desktop typecheck` failed because Bun did not resolve scripts in this shell; direct equivalent commands were used.
- No manual visual verification was performed in the app window.
