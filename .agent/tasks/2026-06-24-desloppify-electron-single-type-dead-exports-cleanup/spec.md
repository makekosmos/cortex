# Desloppify Electron Single-Type Dead Exports Cleanup

Scope:

- `platform/desktop/electron/clipboard-history-store.ts`
- `platform/desktop/electron/window-effects.ts`

Goal:

- Remove only the dead type exports:
  - `ClipboardHistoryRecordMetadata`
  - `KosmosWindowEffects`
- Keep runtime exports used by tests:
  - `defaultClipboardHistorySettings`
  - `classifyClipboardText`
  - `normalizeWindowEffects`
  - `normalizeLegacyBgMaterial`

Validation:

- `bun run --cwd platform/desktop typecheck`
- `bun test platform/desktop/electron/window-effects.test.ts`
- `bun test platform/desktop/electron/clipboard-history-store.test.ts`
- `bunx knip -c knip.json --workspace platform/desktop --include exports --reporter compact --no-progress`
- `bunx desloppify scan --json .`
