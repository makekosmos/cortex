# Evidence

Baseline scan: `score 9`, `findings 373`, `high 216`, `medium 113`, `low 44`

After scan: `score 9`, `findings 360`, `high 203`, `medium 113`, `low 44`

Target file `platform/desktop/shared/ipc-types.ts`:

- `DEAD_EXPORT` high findings before: 24
- `DEAD_EXPORT` high findings after: 11
- Delta: -13
- 11 IPC exports were intentionally kept because external imports exist.
- 13 declarations were made local-only.

Verification:

- `bun run --cwd platform/desktop typecheck` passes.
- `desloppify scan` completed and produced `.agent/tasks/2026-06-24-desloppify-ipc-types-dead-exports-cleanup/desloppify-after.json`.
- `packages/raycast-api/src/components.ts` has 0 `DEAD_EXPORT` findings in the fresh scan.
