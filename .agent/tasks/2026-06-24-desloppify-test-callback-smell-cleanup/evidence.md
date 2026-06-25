# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    429 |   424 |    -5 |
| Critical       |      0 |     0 |     0 |
| High           |    243 |   243 |     0 |
| Medium         |    124 |   124 |     0 |
| Low            |     62 |    57 |    -5 |

Removed scoped findings:

- `RETURN_UNDEFINED` in `EdenSidebar.spec.ts`
- `RETURN_UNDEFINED` in `raycast/view-model.ts`
- 3 `REDUNDANT_RETURN_AWAIT` findings in kext e2e callbacks

## Verification

```powershell
rtk err bun run --cwd platform/desktop typecheck
rtk err bun run --cwd platform/desktop build:js:shell
rtk test bun test tests/unit/raycast-view-model.test.ts
rtk err bunx vitest run tests/components/EdenSidebar.spec.ts --browser=chromium -t "settings button"
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-test-callback-smell-cleanup.json"
```

Results:

- Typecheck: PASS
- Shell JS build: PASS with existing Vite warnings
- Raycast view-model unit test: PASS, 12 tests / 100 expects
- Focused EdenSidebar browser check: PASS
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.

## Notes

`CmEditor.spec.ts` was investigated but left unchanged. Replacing
`Promise.resolve(undefined)` with fail-fast or explicit Promise alternatives
changed the stale-resolution test behavior.

Full browser spec caveats:

- Full `CmEditor.spec.ts` currently fails on an existing preload fixture issue:
  `window.kepler.ark` is unavailable in the `initApp` test.
- Full `EdenSidebar.spec.ts` currently fails on the object-type picker assertion.
  The focused check covering the edited callback passes.

`return await` findings in `safeHandle` and `extension-marketplace` are
intentional because they are inside `catch`/`finally` semantics.
