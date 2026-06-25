# Evidence

Baseline copied from `.agent/tasks/2026-06-24-desloppify-eden-system-types-dead-exports-cleanup/desloppify-after.json`.

Before scan, `model.ts` had 6 `DEAD_EXPORT` findings, all high severity:
`RaycastListDropdownOptionModel`, `RaycastListDropdownSectionModel`, `RaycastFormFieldModel`, `listRoot`, `detailMetadata`, and `gridItems`.

After the edit, only `gridItems` remained. It stayed exported on purpose because `tests/unit/raycast-view-model.test.ts` imports it and the scan may be a false positive there.

Verification:

- `bun run --cwd platform/desktop typecheck` passed
- `bun test tests/unit/raycast-view-model.test.ts` passed
- desloppify scan still exits 1 because the repo has remaining findings, but the target file improved

Mojibake check: the change was modifier-only ASCII text removal; no new non-ASCII text was introduced.
