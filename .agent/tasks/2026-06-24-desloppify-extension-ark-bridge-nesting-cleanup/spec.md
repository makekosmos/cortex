# Desloppify Extension ARK Bridge Nesting Cleanup

## Classification

FULL_LOOP. This is a focused root E2E refactor inside the ongoing desloppify proof loop.

## Goal

Remove the `DEEP_NESTING` finding in `tests/e2e/extension-ark-bridge.spec.ts` without changing the cold-launch race regression being tested.

## Change

- Extracted page console/pageerror collection into `attachPageErrorCollectors` and `collectFuturePageErrors`.
- Hoisted the Eden open JavaScript probe into `INVOKE_EDEN_OPEN_SCRIPT`.
- Extracted launcher polling into `invokeEdenOpenFromLauncher`.
- Kept the same 3s polling window and final assertion on `commands.invoke status`.

## Verification

- PASS: `rtk err powershell -NoProfile -Command "$env:KOSMOS_HEADLESS='1'; $env:KOSMOS_TEST_MODE='1'; bunx playwright test --config playwright.config.ts extension-ark-bridge.spec.ts --list"`
- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-extension-ark-bridge-nesting-cleanup.json"`

The scan exits 1 because repository findings remain, but the targeted deep-nesting finding disappeared.
