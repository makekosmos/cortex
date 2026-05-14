# Evidence

## Changes

- Restored the old visible custom editor caret in [Editor.css](D:/Personal/Hobby/Coding/kosmos/apps/eden/ts/src/Editor.css) by switching `.pm-inline-caret-enabled` back to `caret-color: transparent` and restoring the `.pm-inline-caret-anchor` blink styles.
- Restored a direct devtools path in [main.ts](D:/Personal/Hobby/Coding/kosmos/apps/eden/ts/main/main.ts) by adding an application menu with `toggleDevTools` and a `before-input-event` handler for `F12` / `Ctrl+Shift+I`.

## Verification

- `node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\typescript@5.8.3\node_modules\typescript\lib\tsc.js --noEmit -p D:\Personal\Hobby\Coding\kosmos\apps\eden\ts\tsconfig.json`
  - PASS
- `bun run lint`
  - FAIL in this environment because `oxlint` cannot load `vite.config.ts` as a TypeScript config file.
- `bun run build`
  - FAIL in this environment because Vite externalize-deps hits sandbox `spawn EPERM` while resolving real paths.

## Acceptance Criteria

- AC1: PASS
- AC2: PASS
- AC3: PASS
- AC4: PASS
