# Evidence — Raycast view host vertical slice

Verified at: 2026-06-04T16:55:57+03:00

## Summary

All acceptance criteria are `PASS`.

## Results

### AC1 — Trusted Raycast view runner returns a host snapshot

Verdict: `PASS`

Evidence:

- Added `runRaycastViewCommand` in `shell/electron/raycast/command-runner.ts`.
- Added snapshot normalization in `shell/electron/raycast/view-model.ts`.
- Unit coverage:
  - `tests/unit/raycast-command-runner.test.ts`
  - `tests/unit/raycast-view-model.test.ts`
- Command:
  - `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - Result: 18 pass, 0 fail.

### AC2 — User-installed Raycast view code is refused

Verdict: `PASS`

Evidence:

- `runRaycastViewCommand` rejects `source: "user"` before importing command code.
- This mirrors the no-view security gate and keeps user-installed JS out of Electron main until an isolated runtime exists.
- Command result: same unit run above, 18 pass, 0 fail.

### AC3 — Electron host window and read-only snapshot IPC

Verdict: `PASS`

Evidence:

- Added `shell/electron/raycast/view-host.ts`.
- `shell/electron/main.ts` now invokes `openRaycastViewCommand` for declared `raycast-view` commands.
- Added shared snapshot types in `shell/shared/raycast-ipc.ts`.
- Added preload surface:
  - `window.kepler.raycast.snapshot(sessionId)`
  - `window.kepler.raycast.action(sessionId, action)`
- `kepler:raycast:action` verifies that the IPC sender is the BrowserWindow that owns the Raycast session before executing builtin host actions:
  - `Action.CopyToClipboard`
  - `Action.OpenInBrowser`
  - `Action.Open`
- `bun run shell:typecheck`
  - Result: `tsc --noEmit` passed.

### AC4 — Shell renderer displays List / Detail / ActionPanel actions

Verdict: `PASS`

Evidence:

- Added `shell/src/views/RaycastHostView.vue`.
- Added renderer components under `shell/src/raycast-host/`:
  - `RaycastListView.vue`
  - `RaycastDetailView.vue`
  - `RaycastActionPanel.vue`
  - `model.ts`
  - `markdown.ts`
- `List.Section` titles and `List.EmptyView` fallback text are modeled in
  `shell/src/raycast-host/model.ts` and covered by
  `tests/unit/raycast-view-model.test.ts`.
- Visual verification:
  - Temporary Vite server: `http://127.0.0.1:5199/#raycast-host?session=visual`
  - Copy status screenshot: `D:\Personal\Hobby\Coding\kosmos\.tmp\visual\2026-06-04-raycast-host\raycast-host-copy-status-1000x720.png`
  - Screenshot: `D:\Personal\Hobby\Coding\kosmos\.tmp\visual\2026-06-04-raycast-host\raycast-host-1000x720.png`
  - Result: screenshots show `List.Section` title, capture clicked `Action.CopyToClipboard` and showed `Скопировано`, then clicked `Action.Push` and showed the pushed `Detail` target without visible overlap or overflow.
- `Action.CopyToClipboard`, `Action.OpenInBrowser`, and `Action.Open` are routed through guarded main-process IPC. Direct clipboard contents were covered by the existing command-runner clipboard adapter tests; session-window authorization is implemented in `shell/electron/raycast/view-host.ts`.

### AC5 — Safe markdown rendering

Verdict: `PASS`

Evidence:

- Added `parseRaycastMarkdown` in `shell/src/raycast-host/markdown.ts`.
- `RaycastDetailView.vue` renders headings, paragraphs, lists, and fenced code as Vue text nodes.
- No raw `v-html` is used for Raycast markdown.
- Unit coverage:
  - `tests/unit/raycast-view-model.test.ts`
  - Result: covered in the 18-pass unit run above.

### AC6 — Verification commands

Verdict: `PASS`

Evidence:

- `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - Result: 18 pass, 0 fail.
- `bun run shell:typecheck`
  - Result: passed.
- `bun run docs:sync`
  - Result: regenerated `AGENTS.md`, `CLAUDE.md`, mobile/crate agent docs, and docs-site llms artifacts.
- `bun run docs:check`
  - Result: docs fresh.
- `bun run ark:guard:writes`
  - Result: `ARK write boundary guard passed.`
- `bun run ark:smoke`
  - Result: `ARK smoke matrix passed.`

## Notes

- Several commands initially failed before execution with `windows sandbox: setup refresh failed with status exit code: 1`; they were rerun with scoped escalation per `windows-sandbox`.
- `bun`-based Playwright capture hung without diagnostics after the sandbox failures. The successful visual capture used the bundled Node runtime:
  `C:\Users\kirill\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\bin\node.exe .tmp/visual/2026-06-04-raycast-host/capture.mjs`.
  The temporary local Vite server was stopped after capture.
