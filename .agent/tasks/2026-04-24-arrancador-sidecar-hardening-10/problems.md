# Problems And Fixes

## P1: Source encoding regression during ScanPage extraction

During integration, `src-vue/test/source-encoding.test.ts` reported mojibake in `ScanPage.vue` and `useScanPageState.ts`.

Fix: restored affected Russian UI strings as valid UTF-8 and reran the source encoding and architecture-boundary tests.

Verification: PASS `bunx vitest run --configLoader native --config src-vue/test/vitest.config.mjs src-vue/test/source-encoding.test.ts src-vue/test/architecture-boundaries.test.ts`

## P2: Biome import ordering failure

The extra quality gate `bun run biome:check` failed on import ordering in `scan-sidecar-fallback.test.ts` and `useScanPageState.ts`.

Fix: applied the safe import ordering change manually.

Verification: PASS `bun run biome:check`
