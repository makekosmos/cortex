# Evidence — Eden shared caret integration

## Scope

Relevant files for this pass:

- `apps/eden/ts/src/InlineCaret.ts`
- `apps/eden/ts/src/Editor.vue`
- `apps/eden/ts/src/Editor.css`
- `apps/eden/ts/src/App.vue`
- `apps/eden/ts/tests/app.spec.ts`
- `.omx/context/eden-shared-caret-20260415T000000Z.md`
- `.omx/plans/prd-eden-shared-caret.md`
- `.omx/plans/test-spec-eden-shared-caret.md`
- `.agent/tasks/2026-04-15-eden-shared-caret/spec.md`

## What changed

- Replaced the global shared overlay caret path with an **editor-scoped inline caret** for Eden’s Tiptap/ProseMirror editor.
- Added `InlineCaret` Tiptap extension that uses a ProseMirror `Decoration.widget(...)` at the collapsed selection position.
- Kept the caret visual inline in the text flow and hid the native caret only while the ProseMirror editor is focused.
- Restored native caret during IME composition by toggling editor DOM classes.
- Added a focused regression test proving the inline caret appears only in the editor and disappears when focus moves to the title input.

## Verification

- `bun run lint` ✅
- `bunx tsc --noEmit -p tsconfig.json` ✅
- `bun run build` ✅
- Focused Playwright editor regressions ✅ `5 passed`
  - open existing note
  - zen mode typing path
  - inline caret visibility path
  - typed note flow
  - slash command flow
- `bun run test:e2e` ✅ `20 passed`

## Acceptance mapping

- AC1: PASS — Eden now uses an editor-scoped caret path integrated into the Tiptap/ProseMirror editor surface.
- AC2: PASS — no copied global caret logic was introduced; the new path is an editor extension plus CSS.
- AC3: PASS — lint/typecheck/build/e2e are green.

## Remaining risks

- This pass solves the Eden editor path only. Other potential non-ProseMirror surfaces still use native caret behavior, which is acceptable and more stable.
- We did not convert `packages/kosmos-visuals/components/CustomCaret.vue` itself to widget/decorations; Eden now has the better editor-specific path locally. Converging that back into shared visuals would be a follow-up design decision.
