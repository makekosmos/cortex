# Evidence: Arrancador sidecar hardening and quality pass

## Result

Verification status: PASS

All acceptance criteria from `spec.md` pass against the current codebase.

## Acceptance Criteria

- AC1 PASS: Missing/unavailable sidecar failures are normalized to `ArrancadorSidecarUnavailableError`, and `scanExecutablesStream` falls back to the TypeScript scanner.
- AC2 PASS: Rust sidecar runtime request/response, copy JSONL, progress events, and backup manifests use `serde`/`serde_json`.
- AC3 PASS: Rust tests cover Unicode request, manifest, copy, and restore paths.
- AC4 PASS: Electron main tests cover unavailable sidecar mapping and scan fallback.
- AC5 PASS: ScanPage orchestration is extracted into `useScanPageState` and `scanResults` helpers.
- AC6 PASS: Architecture guard keeps ScanPage below a route-level size budget and blocks direct low-level scan/import orchestration in the page.
- AC7 PASS: Fresh verification commands pass.

## Changed Areas

- `apps/arrancador/electron/main/sidecar/arrancador-sidecar.ts`
- `apps/arrancador/electron/main/sidecar/arrancador-sidecar.test.ts`
- `apps/arrancador/electron/main/services/scan.ts`
- `apps/arrancador/electron/main/services/scan-sidecar-fallback.test.ts`
- `apps/arrancador/sidecar/Cargo.toml`
- `apps/arrancador/sidecar/src/main.rs`
- `apps/arrancador/src-vue/pages/ScanPage.vue`
- `apps/arrancador/src-vue/composables/useScanPageState.ts`
- `apps/arrancador/src-vue/lib/scanResults.ts`
- `apps/arrancador/src-vue/test/architecture-boundaries.test.ts`

## Fresh Verification Commands

Raw logs are stored in `.agent/tasks/2026-04-24-arrancador-sidecar-hardening-10/raw/`.

- PASS `bun run typecheck`
  - Raw: `raw/typecheck.log`
- PASS `bun run build:main`
  - Raw: `raw/build-main.log`
- PASS `bun run build:preload`
  - Raw: `raw/build-preload.log`
- PASS `bun run build:renderer`
  - Raw: `raw/build-renderer.log`
- PASS `bun run test`
  - Raw: `raw/test.log`
  - Summary: 26 test files passed, 77 tests passed.
- PASS `cargo test --manifest-path sidecar/Cargo.toml`
  - Raw: `raw/cargo-test.log`
  - Summary: 6 tests passed.

## Extra Quality Gates

- PASS `bun run biome:check`
  - Raw: `raw/biome-check.log`
- PASS `git diff --check` on task-touched files.

## Notes

- `ScanPage.vue` is now 167 physical lines.
- `useScanPageState.ts` owns scan page orchestration.
- `scanResults.ts` owns scan result list transformations.
