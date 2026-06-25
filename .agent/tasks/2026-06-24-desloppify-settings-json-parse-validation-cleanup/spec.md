# Desloppify Settings JSON Parse Validation Cleanup

## Classification

FULL_LOOP. This is a focused local-settings validation cleanup inside the ongoing desloppify proof loop.

## Goal

Remove two `JSON_PARSE_CAST` findings in local settings loaders without changing persisted settings behavior.

## Change

- `products/eden/src/composables/usePreferences.ts`
  - Parse localStorage JSON as `unknown`.
  - Use `isPreferencePatch` before merging known boolean fields.
- `platform/desktop/src/my-cosmos/graphSettings.ts`
  - Parse localStorage JSON as `unknown`.
  - Merge only known finite numeric fields and known boolean fields into defaults.
- `products/eden/tests/preferences.test.ts`
  - Updated stale `cmEditorEnabled` expectations to current `readerModeEnabled` preference contract discovered while verifying this slice.

## Verification

- PASS: `rtk err bun test products/eden/tests/preferences.test.ts`
- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS: `rtk err bun run --cwd platform/desktop build:extension eden`
- PASS: `rtk err bun run --cwd platform/desktop build:js:shell`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-settings-json-parse-validation-cleanup.json"`

The shell build emitted existing Vite warnings only. The scan exits 1 because repository findings remain, but the two targeted findings disappeared.
