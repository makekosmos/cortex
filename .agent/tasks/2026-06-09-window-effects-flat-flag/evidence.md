# Evidence — 2026-06-09 window effects flat flag

Verified at: 2026-06-09

## AC1 — global env parsing

Verdict: PASS

Evidence:

- `platform/desktop/electron/window-effects.ts` parses `KOSMOS_WINDOW_EFFECTS=flat|mica|acrylic`.
- `flat` maps to Electron material `"none"`.
- Regression test:

```powershell
bun test platform/desktop/electron/window-effects.test.ts
```

Result: PASS — 5 tests, 9 assertions.

## AC2 — requested surfaces use global resolver

Verdict: PASS

Evidence:

`resolveWindowMaterial(...)` is consumed by:

- `platform/desktop/electron/main.ts`
- `platform/desktop/electron/settings-window.ts`
- `platform/desktop/electron/install-extension-window.ts`
- `platform/desktop/electron/extension-host.ts`
- `platform/desktop/electron/focus-widget.ts`
- `platform/desktop/electron/dictation-pill.ts`
- `platform/desktop/electron/focus-overlay.ts`

Grep command:

```powershell
rg "resolveWindowMaterial" platform/desktop/electron -n
```

## AC3 — legacy env remains supported

Verdict: PASS

Evidence:

`resolveWindowMaterial(...)` checks `KEPLER_BG_MATERIAL=mica|none|acrylic` only when `KOSMOS_WINDOW_EFFECTS` is absent. Covered by test `legacy KEPLER_BG_MATERIAL remains supported when global flag is absent`.

## AC4 — regression test for ignored flat / precedence

Verdict: PASS

Evidence:

`platform/desktop/electron/window-effects.test.ts` includes:

- `KOSMOS_WINDOW_EFFECTS=flat resolves to no background material`
- `KOSMOS_WINDOW_EFFECTS takes precedence over legacy KEPLER_BG_MATERIAL`

The first test fails if `flat` is ignored. The second fails if legacy env wins over the global flag.

## AC5 — verification and docs

Verdict: PASS

Commands:

```powershell
bun test platform/desktop/electron/window-effects.test.ts
bun run typecheck   # from platform/desktop
node scripts/sync-agents-docs.mjs
node scripts/check-docs-freshness.mjs
```

Results:

- Unit test: PASS — 5 tests.
- Typecheck: PASS — `tsc --noEmit`.
- Docs sync: PASS — updated generated docs artifacts.
- Docs check: PASS — no stale references.

Notes:

- Initial sandboxed `rtk bun test ...` failed with `EPERM` while reading project files; rerun with approved escalation passed.
- `bun run docs:sync` / `bun run docs:check` did not resolve root scripts in this shell, so the equivalent node scripts were run directly.
