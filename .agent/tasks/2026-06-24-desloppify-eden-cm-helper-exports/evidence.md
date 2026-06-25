# Evidence

## Commands

- `rg CM_SAFE_NODES|CM_SAFE_MARKS|getCmEditorBlockers|markdownListHangingIndentCh|skipOrderedListRenumber|slashCommandRender|resolvePointerTargetElement|isCmSafeDoc|coerceToCmSafeDoc|shouldUseCmEditor|markdownListIndentPlugin|orderedListRenumber|slashCommandSource|shouldStartBlockSelectionTracking products/eden/src products/eden/tests tests`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:extension eden`
- `bun test products/eden/tests/cmGate.test.ts`
- `bunx vitest run --browser=chromium tests/components/BlockSelectionPointer.spec.ts` from `products/eden`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-eden-cm-helper-exports/desloppify-after.json`

## Results

- References check: PASS.
  - `CM_SAFE_NODES`, `CM_SAFE_MARKS`, `markdownListHangingIndentCh`,
    `skipOrderedListRenumber`, and `resolvePointerTargetElement` are only used
    inside their defining files.
  - `getCmEditorBlockers` had no callers and was removed with its private
    collector.
  - `slashCommandRender` had no callers and was removed with its private render
    helper.
  - Editor APIs imported by `CmEditor` and focused tests remain exported.
- `typecheck`: PASS.
- `build:extension eden`: PASS.
- `cmGate.test.ts`: PASS, 15 tests.
- `BlockSelectionPointer.spec.ts`: PASS.
- Baseline scan: score 0, 479 findings; severity critical 0, high 278,
  medium 130, low 71.
- Final scan: score 0, 472 findings; severity critical 0, high 271,
  medium 130, low 71.
- Scoped `DEAD_EXPORT`: 7 -> 0.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Scoped CM/helper exports are removed.
- AC3: PASS. Editor APIs remain exported and focused tests passed.
- AC4: PASS. Relevant checks passed.
