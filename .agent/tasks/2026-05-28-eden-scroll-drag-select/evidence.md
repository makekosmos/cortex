# Evidence — Eden scroll / drag-select stability

Verified on 2026-05-28 against current worktree.

## AC1 — Native text selection ownership

**Verdict: PASS**

Evidence:

- `extensions/eden/src/lib/blockSelectionPointer.ts` returns `false` for descendants inside `.ProseMirror` and input-like controls.
- `extensions/eden/src/Editor.vue:onContentMouseDown` returns before `blockSelection.startTracking(...)` when `shouldStartBlockSelectionTracking(...)` is false.
- `bun run --cwd extensions/eden test:vue` passed `BlockSelectionPointer.spec.ts`:
  - paragraph/span/text-node inside `.ProseMirror` do not start block-selection tracking;
  - input/button controls do not start block-selection tracking.

## AC2 — Block selection entry points

**Verdict: PASS**

Evidence:

- `BlockSelectionPointer.spec.ts` confirms gutter (`.editor-content-area`, `.editor-rail`) and empty `.ProseMirror` surface still return `true`.
- Interactive controls (`button`, `input`) return `false`.

## AC3 — Scroll freeze lifecycle

**Verdict: PASS**

Evidence:

- `extensions/eden/src/lib/blockSelectionClasses.ts` separates:
  - `kepler-block-select-active` for active drag or persisted selection visuals;
  - `kepler-block-drag-active` for active drag only.
- `extensions/eden/src/Editor.css` now applies `overflow:hidden` only via `.kepler-block-drag-active`.
- `BlockSelectionClasses.spec.ts` confirms persisted selection does not set drag class.
- Playwright smoke `node .agent/tasks/2026-05-28-eden-scroll-drag-select/smoke/verify-eden-scroll-drag-select.mjs` returned:

```json
{
  "persistedSelectionOverflowY": "auto",
  "dragOverflowY": "hidden",
  "afterDragOverflowY": "auto",
  "verdict": "PASS"
}
```

## AC4 — Autoscroll bounds

**Verdict: PASS**

Evidence:

- `computeBlockSelectionAutoScrollDelta(...)` is now a pure helper.
- `BlockSelectionAutoScroll.spec.ts` confirms:
  - center pointer returns `0`;
  - top edge returns negative delta;
  - bottom edge returns positive delta.
- `useBlockSelection.autoScrollTick()` still exits when `!active || !hasMoved`, and `finishDrag()` cancels pending rAF.
- `Editor.vue` cleanup removes `mousemove`/`mouseup` listeners and calls `blockSelection.cancelDrag()` on watch cleanup and unmount.
- `TaskRefView.vue` programmatic input focus during sibling navigation / autoFocus uses `{ preventScroll: true }` to avoid browser `focus()`-initiated scroll jumps.

## AC5 — CSS invariant

**Verdict: PASS**

Evidence:

- `rg -n "scroll-padding-(top|bottom)" extensions/eden/src/Editor.css extensions/eden/src/App.css` finds only explanatory comments, no active declarations.
- Playwright smoke loaded real `Editor.css` and observed:

```json
{
  "scrollPaddingTop": "auto",
  "scrollPaddingBottom": "auto"
}
```

## AC6 — Regression coverage

**Verdict: PASS**

Evidence:

- Added/updated tests:
  - `extensions/eden/tests/components/BlockSelectionPointer.spec.ts`
  - `extensions/eden/tests/components/BlockSelectionClasses.spec.ts`
  - `extensions/eden/tests/components/BlockSelectionAutoScroll.spec.ts`
- `bun run --cwd extensions/eden test:vue`: 6 files passed, 22 tests passed.
- `bun run --cwd extensions/eden test:unit`: 20 tests passed.
- Browser plugin runtime failed before tab creation due Windows sandbox setup; fallback Playwright smoke verified real CSS lifecycle in Chromium.

## AC7 — Eden guard coverage

**Verdict: PASS**

Commands:

- `bun run --cwd extensions/eden test:unit` — PASS, 20 tests.
- `bun run --cwd extensions/eden test:vue` — PASS, 6 files / 22 tests.
- `bun run --cwd shell build:extensions` — PASS.
- `bun run --cwd shell build:extensions` after TaskRef `preventScroll` follow-up — PASS.
- `bun run --cwd shell typecheck` — PASS.
- `bun run --cwd shell typecheck` after TaskRef `preventScroll` follow-up — PASS.
- `bun run ark:guard:writes` — PASS.
- `bun run ark:smoke` — PASS, ARK smoke matrix passed.
- `bun run docs:check` — PASS.
- `bun run docs:sync` — PASS.
- final `bun run docs:check` — PASS.

## AC8 — Postmortem/evidence

**Verdict: PASS**

Evidence:

- `docs-site/agents/postmortems.md` has `UPDATE 2026-05-28 — text drag у верхнего края снова уезжает вниз` with symptoms/root cause/fix/regression/prevention.
- This proof-loop folder contains `spec.md`, `evidence.md`, `evidence.json`, and smoke artifacts.
