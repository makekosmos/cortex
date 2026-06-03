# Evidence

## Export

PASS — old GPUI Akasha was committed and pushed to private repo:
`https://github.com/ksanrse/akasha-gpui`.

## Local migration

PASS — `apps/akasha` was removed from the root Cargo workspace.

PASS — `extensions/akasha/manifest.json` now declares:

- `kind: "vue"`
- `entryHtml: "dist/index.html"`
- `devPort: 5185`

PASS — Akasha Vue reader has:

- file input for `.epub`
- ZIP/OPF/spine reader in `src/lib/epub.ts`
- continuous chapter rendering
- table of contents
- font family and font size controls
- local reader-state persistence

PASS — Akasha library UX has:

- local imported book catalog (`library.json`)
- SHA-256 book identity and dedupe-ready storage paths
- binary extension userData API for persisted EPUB files
- Apple Books-style empty/library/continue surfaces
- restore-ready progress model based on chapter/block id

## Verification

PASS — `bun run --cwd shell build:extensions`

PASS — `node shell\node_modules\vite\bin\vite.js build --config extensions\akasha\vite.config.mjs --configLoader native`

PASS — `bun run --cwd shell typecheck`

PASS — `bun run docs:sync`

PASS — `bun run docs:check`

PASS — visual verification via Playwright against shared Vite extension config:
`.tmp/akasha-vue-empty.png`.

PASS — synthetic EPUB open flow via Playwright:
`.tmp/akasha-vue-loaded.png`.

PASS — titlebar/source-serif polish:

- `DesktopChrome` + `WindowControls` from `@kosmos/visuals`
- `Source Serif 4` bundled as Akasha-only dependency
- screenshot: `.tmp/akasha-titlebar-source-serif-fixed.png`

PASS — library visual verification via Playwright:

- empty library: `.tmp/akasha-library-empty-final.png`
- imported book reader: `.tmp/akasha-library-reader-final.png`
- continue card after returning to library: `.tmp/akasha-library-with-book-final.png`

PASS — `bun run ark:smoke`

Note: first earlier `ark:smoke` attempt timed out at 124 seconds; rerun with a
longer timeout passed. During the library UX pass, the first `ark:smoke` retry
was blocked by a locked `target/debug/ark-core-rpc.exe`; stopping that workspace
process and rerunning passed.
