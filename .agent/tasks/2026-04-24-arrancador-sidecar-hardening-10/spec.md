# Task: Arrancador sidecar hardening and quality pass

## Context

Arrancador currently has strong Electron/Vue boundaries, but a fresh `bun run test` fails on Windows because a missing Rust sidecar is surfaced as a generic process error instead of `ArrancadorSidecarUnavailableError`. The Rust sidecar also hand-parses JSON and can corrupt non-ASCII paths. ScanPage remains a large route component with orchestration that should be easier to test and read.

## Acceptance Criteria

AC1. `scanExecutablesWithSidecar` maps both synchronous spawn failures and asynchronous process launch failures for a missing/unavailable sidecar to `ArrancadorSidecarUnavailableError`, and `scanExecutablesStream` falls back to the TypeScript scanner when that happens.

AC2. Rust sidecar request/response and manifest JSON handling uses structured JSON serialization/deserialization rather than handwritten string parsing for runtime requests, copy request lines, and backup manifest entries.

AC3. Rust sidecar tests cover Unicode/UTF-8 paths or JSON payloads so non-ASCII paths do not regress.

AC4. Electron main tests cover sidecar fallback behavior and unavailable sidecar mapping in the current Windows-compatible launch path.

AC5. ScanPage orchestration is more atomic: scan/import stateful behavior is extracted out of the route component into focused composable(s) or helper modules without changing user-facing behavior.

AC6. Architecture guard tests or unit tests cover the new boundaries so route-level scan behavior does not collapse back into a large all-in-one page unnoticed.

AC7. Fresh verification passes on the current codebase:
- `bun run typecheck`
- `bun run build:main`
- `bun run build:preload`
- `bun run build:renderer`
- `bun run test`
- `cargo test --manifest-path sidecar/Cargo.toml`

## Non-goals

- Do not reintroduce Tauri or React runtime paths.
- Do not rewrite unrelated Arrancador features.
- Do not change the public IPC contract except where tests prove an existing bug.
