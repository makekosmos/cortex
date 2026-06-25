# Desloppify Eden Icon Resolver Name Cleanup

## Classification

FULL_LOOP. This is a narrow Eden helper cleanup inside the ongoing desloppify proof loop.

## Goal

Remove the `NUMERIC_SUFFIX` finding in `products/eden/src/lib/iconResolver.ts` without changing generated icon SVGs.

## Change

- Renamed the local `GAMEPAD_2` constant to `GAMEPAD_CONTROLLER`.
- Kept the SVG node data and `"game-controller"` mapping unchanged.

## Verification

- PASS: `rtk err bun test products/eden/tests/systemTypes.test.ts`
- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS: `rtk err bun run --cwd platform/desktop build:extension eden`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-eden-icon-resolver-name-cleanup.json"`

The scan exits 1 because repository findings remain, but the targeted finding disappeared.
