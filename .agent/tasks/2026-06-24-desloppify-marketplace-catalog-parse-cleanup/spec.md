# Desloppify Marketplace Catalog Parse Cleanup

## Classification

FULL_LOOP. This is a focused Electron marketplace catalog validation cleanup inside the ongoing desloppify proof loop.

## Goal

Remove the `JSON_PARSE_CAST` finding in `platform/desktop/electron/extension-marketplace.ts` and strengthen the catalog boundary.

## Change

- Added runtime guards for catalog objects and catalog extension entries.
- Parse `catalog.json` as `unknown`, then validate `schemaVersion`, `updatedAt`, and every extension entry before caching.
- Kept schema version rejection and cache behavior unchanged.

## Verification

- PASS: `rtk err bun test tests/unit/extension-update-plan.test.ts`
- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS: `rtk err bun run --cwd platform/desktop build:js:shell`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-marketplace-catalog-parse-cleanup.json"`

The shell build emitted existing Vite warnings only. The scan exits 1 because repository findings remain, but the targeted finding disappeared.
