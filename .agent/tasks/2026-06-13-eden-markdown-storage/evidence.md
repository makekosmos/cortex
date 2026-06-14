# Evidence — Eden Markdown-as-storage migration

## Overall

PASS_WITH_RESIDUAL_RISK: focused Markdown-storage checks, docs freshness, and Eden build passed; the broader browser component suite still has known unrelated stale failures and was not needed for this proof pass.

## Acceptance criteria

| AC                                                                            | Status | Evidence                                                                                                                                                                                                                                                        |
| ----------------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC1 New/edited entries save markdown content_json                             | PASS   | `CmEditor.vue` now serializes with `JSON.stringify(writeEntryMarkdown(md))`; new/default/import paths in `store/eden.ts`, `kepler-api-shim.ts`, and `GeneralSettings.vue` also use `writeEntryMarkdown`; `CmConvert.spec.ts` asserts markdown adapter behavior. |
| AC2 Existing markdown objects load exact `.text`                              | PASS   | `readEntryMarkdown` returns the `.text` field unchanged for strict markdown envelopes; `content.test.ts` verifies exact text and load→write preservation.                                                                                                       |
| AC3 Legacy PM JSON loads through standalone best-effort reader without TipTap | PASS   | `products/eden/src/editor-cm/content.ts` implements `legacyProseMirrorToText`; Eden grep for `@tiptap`, `mdConvert`, and `createMdConverter` returned no matches.                                                                                               |
| AC4 Legacy smoke coverage incl. image-only source preservation                | PASS   | `content.test.ts` covers nested legacy nodes and confirms image-only legacy content emits `![](src)` when a source attr exists.                                                                                                                                 |
| AC5 Invalid/empty reads empty Markdown, no crash                              | PASS   | `content.test.ts` checks null/undefined/empty/invalid/unknown inputs return `""` without throwing.                                                                                                                                                              |
| AC6 Markdown import/export uses adapter; `mdConvert.ts` removed               | PASS   | `GeneralSettings.vue` now uses `readEntryMarkdown`/`writeEntryMarkdown`; `products/eden/src/editor-cm/mdConvert.ts` is deleted.                                                                                                                                 |
| AC7 TipTap UI/dependencies removed after build/tests prove no live imports    | PASS   | TipTap UI files were deleted, TipTap deps removed from `products/eden/package.json` and `bun.lock`, Eden build passed, and grep found no live `@tiptap` imports.                                                                                                |
| AC8 Writes still use Eden/ARK save paths                                      | PASS   | All write changes stay behind existing `props.onSave`, `window.api.saveEntry`, and ARK shim mapping paths; no direct SQL/migration changes.                                                                                                                     |

## Commands run

1. `rtk bun test products/eden/tests/content.test.ts products/eden/tests/cmGate.test.ts products/eden/tests/obsidianVault.test.ts` — PASS (24 pass / 0 fail).
2. `rtk bun test products/eden/tests/components/CmConvert.spec.ts products/eden/tests/content.test.ts` — PASS (5 pass / 0 fail).
3. `rtk bun run products:build` — PASS; Eden and other extension builds completed.
4. `rtk bun run docs:check` — PASS; no stale docs references.
5. `rtk grep -R "@tiptap\|mdConvert\|createMdConverter" -n products/eden/src products/eden/tests products/eden/package.json bun.lock || true` — PASS; no matches.

## Residual risks

- The full Eden browser component suite still has known unrelated stale failures and was not required for the storage proof pass.
- No manual desktop smoke against a live ARK vault was performed in this run.

## Diff summary

Replaced Eden body storage handling with a Markdown envelope adapter, rewired CM/settings/store/ARK-shim paths to use it, removed legacy TipTap editor code and dependencies, and updated docs plus proof artifacts.
