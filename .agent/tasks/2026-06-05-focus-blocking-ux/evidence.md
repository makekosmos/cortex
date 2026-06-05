# Evidence — Focus blocking UX repair

Date: 2026-06-05

## Acceptance Criteria

- AC1 PASS by code/type coverage — Focus block suggestions no longer show `exec_path`; selected apps render user-facing name/icon pills.
- AC2 PASS by unit/type/Rust coverage — payload carries `blockedApps`; backend stores `blocked_apps`; launcher checks exact id and normalized display name.
- AC3 PASS by code/type coverage — selected task is now a separate removable pill; goal text remains free-form and task mention removal does not depend on the task title staying inside the text input.
- AC4 PASS by code review — mention popover now uses `var(--shadow-floating)` instead of foreground-derived light glow.
- AC5 PASS by code review — Focus widget no longer renders the shield/protection icon.
- AC6 PASS by code/type coverage — blocked app launch shows overlay with a 3-second hover timer before enabling 5-minute snooze; snooze launches the app and stores a local 5-minute bypass key.
- AC7 PARTIAL — focused unit/type/Rust checks passed. Visual verification was attempted but blocked by local Vite dev server instability.

## Commands

- PASS `bun test tests/unit/focus-command-payload.test.ts tests/unit/focus-app-blocking.test.ts tests/unit/focus-command-instant-render.test.ts`
- PASS `bun run shell:typecheck`
- PASS `$env:CARGO_TARGET_DIR='.tmp\cargo-focus-app-test'; cargo test -p kepler-backend set_then_get_active_state_round_trip`

## Visual Verification Attempt

Attempted a self-contained Playwright script that starts shell Vite on a dedicated port, injects narrow `window.kepler` mocks, and verifies:

- task pill selection;
- app pill selection;
- `blockedApps` metadata in start payload;
- blocked overlay for a launcher app with the same display name and different id;
- 3-second hover snooze permitting launch.

Blocked by environment:

- Direct `bunx vite --host 127.0.0.1 --port 5199/5200 --strictPort` served initial HTML, then module requests failed with `ERR_CONNECTION_RESET` / `ERR_CONNECTION_REFUSED`; `.launcher` never mounted.
- `bun run --cwd shell dev:kepler` reported `http://localhost:5174/`, but no matching TCP listener remained; only unrelated existing listeners on 5173/5185 were present.

No screenshot artifact is trustworthy for this attempt.

## Not Run

- `bun run lint`
- `bun run ark:guard:writes`
- `bun run ark:smoke`
- `bun run docs:sync`
- `bun run docs:check`

Per user instruction on 2026-06-05: full checks and docs sync/check are reserved for commit/push or explicit final pre-commit verification, not every iteration.
