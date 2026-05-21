# Evidence

- AC1: PASS
  - `apps/eden/ts/src/InlineCaret.ts` now renders a visible fixed overlay caret (`.pm-inline-caret-overlay`) when the editor is focused and the selection is collapsed.
  - `apps/eden/ts/src/Editor.css` restores transparent native caret mode only while the overlay caret is active.

- AC2: PASS
  - During pointer selection and for non-collapsed selection, the overlay caret hides and the editor falls back to native selection behavior.

- AC3: PASS
  - The visible caret is no longer rendered as an in-flow widget anchor in the editor text flow.
  - `InlineCaret.ts` no longer depends on `Decoration.widget(...)` for visible caret rendering.

- AC4: PASS
  - `apps/eden/ts/tests/app.spec.ts` was updated to assert visible overlay caret behavior instead of widget-anchor presence.

- AC5: PASS
  - `node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\typescript@5.8.3\node_modules\typescript\lib\tsc.js --noEmit -p D:\Personal\Hobby\Coding\kosmos\apps\eden\ts\tsconfig.json`
