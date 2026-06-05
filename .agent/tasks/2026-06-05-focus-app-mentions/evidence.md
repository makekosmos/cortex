# Evidence — Focus app mentions

Date: 2026-06-05

## Acceptance Criteria

- AC1 PASS — Focus Session "Блокировка" renders as a textarea-style app mention field. Selecting `@Steam` adds an `@Steam` chip.
- AC2 PASS — `buildFocusSessionStartInput` carries `blockedAppIds`; `focus-session.ts` maps it into backend active state as `blocked_app_ids`; Rust round-trip stores non-empty ids.
- AC3 PASS — `LauncherView` checks active focus state before `app_index.launch`; blocked app ids show a Russian error and skip the launch call.
- AC4 PASS — domain `blocklist_id` path is still present in active state and `focus-session.ts` still applies hosts blocking through the existing helper/service flow only.
- AC5 PASS — verification commands and visual artifacts are listed below.

## Visual Verification

- `.tmp/visual/2026-06-05-focus-app-mentions/focus-app-mention-720x460.png`
- `.tmp/visual/2026-06-05-focus-app-mentions/launcher-blocked-app-720x460.png`

Scenario:

- mocked `app_index.list_all` with `Steam` and `Discord`;
- opened Focus command panel;
- typed `@st`, selected `Steam`, verified `@Steam` chip;
- started focus and verified `blockedAppIds` contains `steam`;
- reloaded launcher with active focus `blocked_app_ids: ["steam"]`;
- clicked `Steam` app command, verified Russian blocked message and zero `app_index.launch` calls.

## Commands

- PASS `bun run lint`
- PASS `bun test tests/unit/focus-command-payload.test.ts tests/unit/focus-app-blocking.test.ts tests/unit/focus-command-instant-render.test.ts`
- PASS `bun run shell:typecheck`
- PASS `$env:CARGO_TARGET_DIR='.tmp\cargo-focus-app-test'; cargo test -p kepler-backend set_then_get_active_state_round_trip`
- PASS `bun run ark:guard:writes`
- PASS `bun run ark:smoke`
- PASS `bun run docs:sync`
- PASS `bun run docs:check`

Notes:

- The first `ark:smoke` attempt failed because the visual-verification dev server held `target\debug\ark-core-rpc.exe` (`os error 5`). After stopping the dev watcher/backend that I started for screenshots, the same smoke command passed.
- Scope remains launcher-level app blocking. This does not kill already-running processes and does not block apps launched outside Kosmos.
