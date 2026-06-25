# Evidence

Changed only two type declarations:

- `ClipboardHistoryRecordMetadata` is no longer exported in `clipboard-history-store.ts`
- `KosmosWindowEffects` is no longer exported in `window-effects.ts`

Kept the runtime/test seams exported:

- `defaultClipboardHistorySettings`
- `classifyClipboardText`
- `normalizeWindowEffects`
- `normalizeLegacyBgMaterial`

Checks:

- `rtk err bun run --cwd platform/desktop typecheck` passed
- `rtk test bun test platform/desktop/electron/window-effects.test.ts` passed
- `rtk test bun test platform/desktop/electron/clipboard-history-store.test.ts` passed
- `rtk bunx knip -c knip.json --workspace platform/desktop --include exports --reporter compact --no-progress` still reports unrelated exports, but no longer reports the two target type exports
- `rtk proxy cmd /c "set PATH=%CD%\\.tmp\\bin;%PATH%&& bunx desloppify scan --json . > .tmp\\desloppify-after-electron-single-type-dead-exports-cleanup.json"` completed with exit code 1, and the copied scan JSON no longer contains `ClipboardHistoryRecordMetadata` or `KosmosWindowEffects`

Scan delta from the copied baseline:

- findings: 317 -> 315
- high: 160 -> 158
- dead-code: 163 -> 161
