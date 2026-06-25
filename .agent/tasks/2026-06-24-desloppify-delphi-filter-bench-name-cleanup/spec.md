# Desloppify Delphi Filter Benchmark Name Cleanup

## Classification

FULL_LOOP. This is a narrow benchmark-only cleanup inside the ongoing desloppify proof loop.

## Goal

Remove the `NUMERIC_SUFFIX` finding in `products/delphi/tests/filterService.bench.ts` without changing benchmark behavior or output columns.

## Change

- Renamed local timing variable `t0` to `startedAt`.
- Renamed local percentile variable to `tailLatencyMs`.
- Kept the `p99_ms` result field and printed table column unchanged.

## Verification

- PASS: `rtk err bun test products/delphi/tests/filterService.test.ts`
- PASS: `rtk err bun run products/delphi/tests/filterService.bench.ts`
- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-delphi-filter-bench-name-cleanup.json"`

The scan exits 1 because repository findings remain, but the targeted finding disappeared.
