# Desloppify Focus JSON Response Validation Cleanup

## Classification

FULL_LOOP. This touches focus-mode helper/service response boundaries, so focus-mode docs and forbidden rules were loaded first.

## Goal

Remove two `JSON_PARSE_CAST` findings in focus-mode helper/service response parsing without changing hosts-file side effects, service timeouts, widget behavior, or pomodoro lifecycle.

## Change

- `platform/desktop/electron/focus-block.ts`
  - Added `HelperResponse` shape validation.
  - Direct and elevated helper responses now parse as `unknown` before validation.
- `platform/desktop/electron/focus-service.ts`
  - Added `CliResult` and `ServiceResponse` shape validation.
  - CLI and named-pipe responses now parse as `unknown` before validation.

No focus mode side-effect paths were changed: no hosts writes, no service timeout changes, no widget lifecycle changes.

## Verification

- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS: `rtk err bun run --cwd platform/desktop build:js:shell`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-focus-json-response-validation-cleanup.json"`

The shell build emitted existing Vite warnings only. The scan exits 1 because repository findings remain, but the two targeted findings disappeared and the score improved from 0 to 3.
