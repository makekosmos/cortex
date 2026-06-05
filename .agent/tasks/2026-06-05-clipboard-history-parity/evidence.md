# Evidence — Clipboard History Raycast Parity

Verified at: 2026-06-05T11:15:00+03:00

## Summary

All acceptance criteria are `PASS`.

## External Reference

Checked on 2026-06-05:

- Raycast Clipboard History page: text, images, colors, links, files, search/type filters, pinning, copy/open/remove/clear, detail metadata.
- Raycast API clipboard/actions docs: clipboard history access and copy opt-out exist, but OS paste and persistence are out of scope for this task.

## Results

### AC1 — Store models Raycast-like item types

Verdict: `PASS`

Evidence:

- `ClipboardHistoryItem` now supports `text | image | link | color | file`, `pinned`, `updatedAt`, `searchText`, URL/color/file/image metadata.
- `tests/unit/clipboard-history-store.test.ts` covers text normalization, link/color classification, file records, image records, duplicate timestamp behavior, max item pruning, pin ordering, and clear semantics.

### AC2 — Capture classifies copied content

Verdict: `PASS`

Evidence:

- `shell/electron/clipboard-history.ts` records text, images, and best-effort file paths from Electron clipboard formats.
- Text classification is covered by `classifyClipboardText` tests for link and color values.

### AC3 — Actions support copy/open/pin/delete/clear

Verdict: `PASS`

Evidence:

- IPC supports `copy`, `open`, `togglePin`, `delete`, `clear` (unpinned), and `clearAll`.
- Store tests verify pinned entries stay at the top and survive normal clear.

### AC4 — Launcher clipboard mode parity

Verdict: `PASS`

Evidence:

- Launcher clipboard mode supports all type filters: all, text, image, link, color, file.
- Keyboard shortcuts added for open (`Ctrl+O`), pin (`Ctrl+Shift+P`), remove (`Ctrl+X`/Delete), clear all (`Ctrl+Shift+X`), and copy (`Enter`).
- Visual screenshot:
  - `.tmp/visual/2026-06-05-clipboard-history-parity/clipboard-launcher-1100x760.png`
  - `.tmp/visual/2026-06-05-clipboard-history-parity/clipboard-launcher-color-filter-1100x760.png`

### AC5 — Standalone Clipboard History parity

Verdict: `PASS`

Evidence:

- Standalone view now reuses `ClipboardQuickPanel`, so it exposes the same list/detail/action surface as launcher mode.
- Visual screenshot:
  - `.tmp/visual/2026-06-05-clipboard-history-parity/clipboard-standalone-1100x760.png`

### AC6 — Verification commands

Verdict: `PASS`

Evidence:

- `bun run shell:typecheck`
  - Result: passed.
- `bun test tests/unit/clipboard-history-store.test.ts tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts`
  - Result: 42 pass, 0 fail.
- `bun run lint`
  - Result: passed.
- `bun run ark:guard:writes`
  - Result: `ARK write boundary guard passed.`
- `bun run ark:smoke`
  - First run failed because existing workspace process `ark-core-rpc` locked `target\debug\ark-core-rpc.exe` with `os error 5`.
  - Process found: PID 22356, path `D:\Personal\Hobby\Coding\kosmos\target\debug\ark-core-rpc.exe`.
  - Stopped that workspace process and reran.
  - Result: `ARK smoke matrix passed.`

### AC7 — Merge after AC1-AC6

Verdict: `PASS`

Evidence:

- AC1-AC6 are PASS.
- Merge can proceed after this evidence is committed or kept in the branch diff.
