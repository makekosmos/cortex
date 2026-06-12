# Evidence — Eden Obsidian vault import/export

Verified at: 2026-06-12T15:08:00Z

## Results

| AC  | Verdict | Evidence                                                                                                                                                                                                                                                                                                |
| --- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC1 | PASS    | `products/eden/src/lib/obsidianVault.ts` imports vault Markdown files into entry drafts, accepts files without frontmatter, preserves body Markdown/frontmatter props, extracts `[[wikilinks]]`, and Eden UI saves via `window.api.saveEntry`. `rtk proxy bun test tests/obsidianVault.test.ts` passed. |
| AC2 | PASS    | `buildObsidianExportFiles` creates safe `.md` files with YAML frontmatter through `buildEntryMarkdownDocument`; `window.api.exportMarkdownVault(files)` writes them to a user-selected folder. `rtk proxy bun test tests/obsidianVault.test.ts` passed.                                                 |
| AC3 | PASS    | `SYSTEM_TYPE_IMAGE` / `SYSTEM_TYPE_IMAGE_ID` added with image metadata fields and icon mapping. `rtk proxy bunx vitest run tests/systemTypes.test.ts --config vite.config.mjs` passed.                                                                                                                  |
| AC4 | PASS    | Vault scan returns supported local image metadata; import creates `image_obj` entries and rewrites Markdown image refs to local `file://` URLs for display. Covered by Obsidian import tests and Eden build.                                                                                            |
| AC5 | PASS    | CodeMirror live preview renders Markdown image syntax through `ImagePreviewWidget`; `mdConvert` preserves image markdown across save/reopen. `rtk proxy bunx vitest run tests/components/CmConvert.spec.ts --config vite.config.mjs` passed.                                                            |
| AC6 | PASS    | Unit tests, ARK write-boundary guard, docs freshness, whitespace check, and Eden extension build passed.                                                                                                                                                                                                |

## Commands

- `rtk proxy bunx vitest run tests/components/CmConvert.spec.ts tests/systemTypes.test.ts --config vite.config.mjs` — PASS, 8 tests.
- `rtk proxy bun test tests/obsidianVault.test.ts` — PASS, 3 tests.
- `rtk proxy node scripts/check-ark-write-boundaries.mjs` — PASS.
- `rtk proxy node scripts/check-docs-freshness.mjs` — PASS.
- `rtk proxy node scripts/build-extensions.mjs --only eden` from `platform/desktop` — PASS.
- `rtk git diff --check` — PASS.

## Delegation

- `Fermat` explorer (`gpt-5.3-codex-spark`) inspected existing Markdown/frontmatter and extension IPC.
- `Hypatia` explorer (`gpt-5.3-codex-spark`) inspected object types, image fields, and CM editor image gaps.
- `Newton` worker (`gpt-5.4-mini`) implemented the `Изображение` system type slice and tests.
- `Gibbs` worker (`gpt-5.4-mini`) implemented the pure Obsidian vault library/tests; parent integrated and extended it with image asset handling.
