# Desloppify Akasha JSON Parse Validation Cleanup

## Classification

FULL_LOOP. This is a focused Akasha storage validation cleanup inside the ongoing desloppify proof loop.

## Goal

Remove two `JSON_PARSE_CAST` findings in Akasha local/userData storage parsing while preserving fallback behavior.

## Change

- `normalizeCatalog` now accepts `unknown` and validates the catalog container before normalizing books.
- Akasha library fallback JSON is parsed as `unknown` and passed through `normalizeCatalog`.
- Reader settings userData/fallback JSON is parsed as `unknown` and passed through `normalizeSettings`.

## Verification

- PASS: `rtk err bun run --cwd platform/desktop build:extension akasha`
- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-akasha-json-parse-validation-cleanup.json"`

The scan exits 1 because repository findings remain, but the two targeted findings disappeared.
