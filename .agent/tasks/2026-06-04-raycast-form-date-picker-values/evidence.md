# Evidence

## Implementation

- Added typed `Form.DatePicker` props in `packages/raycast-api/src/components.ts`.
- `serializableProp(...)` now serializes valid `Date` props to ISO strings.
- DatePicker `onChange` callbacks now receive `Date | null` from host payloads.
- Host form model converts DatePicker defaults/values to `YYYY-MM-DD` for the native date input.
- Updated Raycast compatibility docs/roadmap to mention DatePicker defaults/onChange support.

## Verification

- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-view-model.test.ts` (14 pass, 112 expectations).
- PASS: `bun run shell:typecheck`.
- PASS: `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-view-model.test.ts` (31 pass, 165 expectations).
- PASS: `bun run docs:sync`.
- PASS: `bun run docs:check`.
- PASS: `bun run ark:guard:writes`.
- PASS: `$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\.tmp\cargo-ark-smoke'; bun run ark:smoke`.
  - Known warnings remain: `collect_state_events` is unused and `inlineDynamicImports` is ignored with code splitting.
