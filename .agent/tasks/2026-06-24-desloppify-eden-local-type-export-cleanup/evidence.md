# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    442 |   440 |    -2 |
| Critical       |      0 |     0 |     0 |
| High           |    245 |   243 |    -2 |
| Medium         |    127 |   127 |     0 |
| Low            |     70 |    70 |     0 |

Code delta:

- `products/eden/src/components/sidebar/types.ts`
- `products/eden/src/composables/useBlockSelection.ts`
- `2 files changed, 4 deletions(-)`

## Change

Removed two unused type exports:

- `DragPayload`
- `UseBlockSelectionReturn`

Runtime behavior was left unchanged.

## Verification

```powershell
rtk err bun run --cwd platform/desktop typecheck
rtk err bun run --cwd platform/desktop build:extension eden
rtk err bunx vitest run tests/components/BlockSelectionAutoScroll.spec.ts --browser=chromium
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-eden-local-type-export-cleanup.json"
```

Results:

- Typecheck: PASS
- Eden extension build: PASS
- Focused Vitest Browser spec: PASS
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.

## Notes

Remaining nearby findings were not removed in this slice:

- `sortEntries` is imported by `products/eden/src/components/spaces/SpacesView.vue`.
- `DragRect` is imported by `products/eden/src/components/BlockSelectionOverlay.vue`.
- `useBlockSelection` itself needs separate editor/UI behavior proof before removal or refactor.

The first attempted focused command used bare `bun test` and failed with
`document is not defined`; the correct check is the Eden Vitest Browser runner.
