# Evidence — Raycast Form host vertical slice

Verified at: 2026-06-04T18:03:15+03:00

## Summary

All acceptance criteria are `PASS`.

## Results

### AC1 — Form root renders in Raycast host

Verdict: `PASS`

Evidence:

- `shell/src/views/RaycastHostView.vue` routes `snapshot.root.type === "Form"` to `RaycastFormView`.
- Visual screenshot:
  - `D:\Personal\Hobby\Coding\kosmos\.tmp\visual\2026-06-04-raycast-host\raycast-form-submit-1000x720.png`

### AC2 — Form fields/options model

Verdict: `PASS`

Evidence:

- `shell/src/raycast-host/model.ts` exposes `formModel`.
- Unit coverage:
  - `tests/unit/raycast-view-model.test.ts`
  - Result from `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`: 20 pass, 0 fail.

### AC3 — Submit callback lifecycle

Verdict: `PASS`

Evidence:

- `shell/electron/raycast/view-model.ts` registers callback ids for `onAction` and `onSubmit`.
- `shell/electron/raycast/view-host.ts` stores callbacks per session and requires the IPC sender to be the owning Raycast host window before invoking callbacks.
- Unit coverage:
  - `tests/unit/raycast-command-runner.test.ts`
  - Result: included in the 20-pass unit run above.

### AC4 — Renderer UI

Verdict: `PASS`

Evidence:

- Added `shell/src/raycast-host/RaycastFormView.vue`.
- UI text fallbacks are Russian: `Нет полей`, `Отправлено`, `Не удалось отправить`, `Отправить`.
- CSS uses `var(--*)` tokens and no raw `v-html`.
- Visual screenshot shows TextField, TextArea, Checkbox, Dropdown, submit action, and `Отправлено` status.

### AC5 — Verification commands

Verdict: `PASS`

Evidence:

- `bun test tests/unit/raycast-api.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-view-model.test.ts tests/unit/extension-permissions.test.ts`
  - Result: 20 pass, 0 fail.
- `bun run shell:typecheck`
  - Result: passed.
- Visual capture:
  - Result: passed.
- `bun run docs:sync`
  - Result: regenerated agent docs and llms artifacts.
- `bun run docs:check`
  - Result: docs fresh.
- `bun run ark:guard:writes`
  - Result: `ARK write boundary guard passed.`
- `bun run ark:smoke`
  - Result: `ARK smoke matrix passed.`

## Notes

- The temporary Vite server used for Form visual capture was started with PID `20976`. Initial cleanup was delayed by an automatic approval rejection due usage limits plus a sandbox setup failure, then completed on the next continuation.
- Several commands initially failed before execution with `windows sandbox: setup refresh failed`; they were rerun with scoped escalation.
