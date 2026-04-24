# Evidence: Worker 1 AC2/AC3

## Scope

- AC2: Rust sidecar structured JSON handling for requests, copy request JSONL entries, progress/events/responses, and backup manifest entries.
- AC3: Rust sidecar Unicode/UTF-8 request path and manifest/copy/restore JSON coverage.

## Current Code Verification

- `apps/arrancador/sidecar/src/main.rs` now uses serde/serde_json request, response, event, copy request, and manifest structs instead of handwritten JSON field parsing, escaping, or manifest assembly.
- Existing Electron protocol field names are preserved with serde renames: `operation`, `requestPath`, `backupRoot`, `rootLabel`, `relativePath`, `backupPath`, `originalPath`, `totalBytes`, and `file_name`.
- Unit tests cover escaped Unicode request paths, copy request JSONL parsing with non-ASCII values, manifest parsing with escaped Unicode paths, and a copy/restore round trip with non-ASCII paths.

## Commands

- `cargo test --manifest-path apps/arrancador/sidecar/Cargo.toml`: PASS

## AC Status

- AC2: PASS
- AC3: PASS
