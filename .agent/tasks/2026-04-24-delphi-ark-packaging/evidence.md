# Evidence

Task: `2026-04-24-delphi-ark-packaging`

Verification date: 2026-04-24

## Acceptance Criteria

### AC1

PASS. `apps/delphi/ts/package.json` now defines:

```json
"build:sidecar": "cargo build --release --manifest-path ../../../packages/ark-core/rust/Cargo.toml --bin ark-core-rpc"
```

Raw evidence:

- `package-json-evidence.txt`
- `package-json-assert.txt`
- `build-sidecar-release.txt`

### AC2

PASS. `apps/delphi/ts/package.json` now defines:

```json
"build:sidecar:dev": "cargo build --manifest-path ../../../packages/ark-core/rust/Cargo.toml --bin ark-core-rpc"
```

Raw evidence:

- `package-json-evidence.txt`
- `package-json-assert.txt`
- `build-sidecar-dev.txt`

### AC3

PASS. Electron Builder `extraResources` now copies:

```json
{
  "from": "../../../packages/ark-core/rust/target/release/ark-core-rpc.exe",
  "to": "ark-core/ark-core-rpc.exe"
}
```

The configured `extraResources` no longer references `sidecar/target/release/delphi-db.exe`.

Raw evidence:

- `package-json-evidence.txt`
- `package-json-assert.txt`
- `git-diff.txt`

### AC4

PASS. Fresh verification commands were run against the current codebase:

- `bun run build:sidecar`
- `bun run build:sidecar:dev`
- JSON assertion over `apps/delphi/ts/package.json`
- `git diff --check -- apps/delphi/ts/package.json .agent/tasks/2026-04-24-delphi-ark-packaging/spec.md`

Raw evidence:

- `build-sidecar-release.txt`
- `build-sidecar-dev.txt`
- `package-json-assert.txt`
- `git-diff-check.txt`

## Notes

Both Rust builds completed with exit code `0`. Cargo emitted an existing warning that `relay_url` and `relay_api_key` fields are never read in `ark-core-rpc`; that warning is unrelated to this packaging fix and matches the broader audit's relay finding.
