# Evidence — 2026-06-15 Eden CM autosave data loss

Verified at: 2026-06-15T14:12:00+03:00

## AC1

Statement: editor/store save path persists edited body even after optimistic parent draft state.

Command:

```powershell
rtk bunx vitest run tests/components/CmEditor.spec.ts
```

Result: PASS. `products/eden/tests/components/CmEditor.spec.ts` passed 15 tests, including `store handleSave сохраняет body после optimistic updateEntryDraft`.

## AC2

Statement: leaving/unmounting before debounce must not lose the latest body.

Evidence: PASS by code path. `CmEditor.vue::onBeforeUnmount` still calls `flushSave()`, and `eden.handleSave()` no longer drops the save because `entries.value` already contains the optimistic draft. The same store regression covers the formerly skipped persistence call.

## AC3

Statement: regression test covers optimistic parent draft scenario.

Command:

```powershell
rtk bunx vitest run tests/components/CmEditor.spec.ts -t "store handleSave"
```

Result before fix: FAIL, `saveEntry` was called 0 times.  
Result after fix: PASS, 1 test passed.

## AC4

Statement: no direct SQL writes; existing ARK boundary remains.

Command:

```powershell
node scripts/check-ark-write-boundaries.mjs
```

Result: PASS, `ARK write boundary guard passed.`

## AC5

Statement: postmortem documents root cause and prevention.

Command:

```powershell
node scripts/check-docs-freshness.mjs
```

Result: PASS, docs freshness check reported no stale references.

## Additional Checks

```powershell
bun run lint
bunx oxfmt --check products/eden/src/store/eden.ts products/eden/tests/components/CmEditor.spec.ts products/eden/src/editor-cm/CmEditor.vue products/eden/src/editor-cm/cm/live-preview.ts
```

Results: PASS.

## Known Unrelated Blockers

Full `products/eden` test suite was previously observed blocked by existing unrelated failures in `preferences.test.ts`, `vimMotions.test.ts`, and `VimSettings.spec.ts`. The targeted editor/store regression and changed-file checks passed.
