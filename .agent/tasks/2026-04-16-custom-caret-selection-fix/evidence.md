# Evidence

## Acceptance Criteria

- AC1: PASS
  - `packages/kosmos-visuals/components/CustomCaret.vue` still renders the shared custom caret for focused collapsed caret states.
  - The custom caret is not removed; it still owns collapsed caret rendering through `render()` and `getCaretRect()`.

- AC2: PASS
  - `CustomCaret.vue` now detects expanded selection state via `hasExpandedSelection()`.
  - When selection is expanded, `syncNativeCaretVisibility()` restores the native caret mode and the custom caret hides because no caret rect is returned for expanded selections.
  - During mouse drag selection, the handoff now starts earlier on `pointerdown`, so the component does not wait for a later expanded-selection event before restoring native behavior.

- AC3: PASS
  - On selection collapse, `syncNativeCaretVisibility()` switches back to transparent native caret mode and the shared custom caret renders again without blur/refocus.
  - This handoff is driven from `pointerdown`, `pointerup`, `selectionchange`, and `render()`, so it reacts both to double-click selection and to press-drag selection.

- AC4: PASS
  - `apps/eden/ts/tests/app.spec.ts` now contains a regression test: `should restore native selection mode while text is selected in title input`.
  - The test covers pointerdown handoff into native selection mode, expanded-selection behavior, and the return to collapsed custom-caret mode.

- AC5: PASS
  - TypeScript verification passed for the touched consumers of `@kosmos/visuals`:
    - `apps/eden/ts`
    - `apps/delphi/ts`
    - `apps/dashboard`

## Commands

- PASS: `node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\typescript@5.8.3\node_modules\typescript\lib\tsc.js --noEmit -p D:\Personal\Hobby\Coding\kosmos\apps\eden\ts\tsconfig.json`
- PASS: `node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\typescript@5.8.3\node_modules\typescript\lib\tsc.js --noEmit -p D:\Personal\Hobby\Coding\kosmos\apps\delphi\ts\tsconfig.json`
- PASS: `node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\typescript@5.8.3\node_modules\typescript\lib\tsc.js --noEmit -p D:\Personal\Hobby\Coding\kosmos\apps\dashboard\tsconfig.json`

## Notes

- I did not run Playwright in this sandbox because prior Electron/Playwright runs in this environment are blocked by process-spawn restrictions.
- The regression test was added at the app level because Eden already had live caret E2E coverage and uses the shared `CustomCaret`.
