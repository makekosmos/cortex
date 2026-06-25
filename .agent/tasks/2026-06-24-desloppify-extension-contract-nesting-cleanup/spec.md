# Desloppify Extension Contract Nesting Cleanup

## Classification

FULL_LOOP. This is an E2E contract refactor, so testing/headless docs were already loaded for this turn.

## Goal

Remove `DEEP_NESTING` findings in `tests/e2e/extensions-contract.spec.ts` while preserving manifest-driven extension contract behavior.

## Change

- Extracted command registry checks into `registeredCommandIds` and `expectContractCommands`.
- Extracted ARK smoke round-trip into `runSmokeRoundTrip` and `expectSmokeRoundTrip`.
- Simplified the generated test body to orchestration plus helper calls.
- Preserved the same command-list assertion and ARK upsert/get/delete smoke semantics.

## Verification

- PASS: `rtk err powershell -NoProfile -Command "$env:KOSMOS_HEADLESS='1'; $env:KOSMOS_TEST_MODE='1'; bunx playwright test --config playwright.config.ts extensions-contract.spec.ts --list"`
- PASS: `rtk err powershell -NoProfile -Command "$env:KOSMOS_HEADLESS='1'; $env:KOSMOS_TEST_MODE='1'; bunx playwright test --config playwright.config.ts extensions-contract.spec.ts -g 'extension contract discovery finds manifests with tests'"`
- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-extension-contract-nesting-cleanup.json"`

The scan exits 1 because repository findings remain, but all four targeted deep-nesting findings in the contract spec disappeared.
