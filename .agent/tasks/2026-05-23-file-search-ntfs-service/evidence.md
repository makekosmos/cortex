# Evidence

## AC1 - Normal backend spawn

PASS

- Reverted the backend-wide UAC spawn path from `shell/electron/main.ts`.
- `spawnBackend` is back to normal `child_process.spawn(exe, [], { env: ... })`.

## AC2 - Service NTFS operation

PASS

- `services/kepler-focus-svc/src/protocol.rs` exposes `ntfs_scan`.
- `services/kepler-focus-svc/src/ntfs_scan.rs` uses `ntfs-reader` MFT scan and
  returns `{ path, name, mtime }` entries.
- `KeplerFocusSvc` was reinstalled from
  `D:\Personal\Hobby\Coding\kosmos\target\release\kepler-focus-svc.exe`; SCM
  now reports that path and service is running.
- Pipe smoke:
  - `{"op":"ping"}` returns `{"ok":true,"pong":true}`.
  - `{"op":"ntfs_scan","root":"","exclude_noisy":true}` returns
    `invalid drive root`, proving the new op is recognized.

## AC3 - Backend uses service first

PASS

- `services/kepler-backend/src/file_index/scanner/ntfs.rs` first opens
  `\\.\pipe\kepler-focus-svc` and sends `ntfs_scan`.
- If the pipe/service is unavailable, it falls back to local `ntfs-reader`;
  existing walk fallback remains in `scanner.rs`.

## AC4 - Noisy-folder filtering

PASS

- `ntfs_scan` accepts `exclude_noisy`.
- The service filters the same noisy folder names before returning records.

## AC5 - Verification

PASS

```powershell
bun run --cwd shell typecheck
cargo test --manifest-path services\kepler-focus-svc\Cargo.toml --lib
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index
$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\target-backend-check'; cargo build --manifest-path services\kepler-backend\Cargo.toml --bin kepler-backend
$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\target-focus-svc-check'; cargo build --release --manifest-path services\kepler-focus-svc\Cargo.toml --bin kepler-focus-svc
```

Result: all passed.

Note: building directly to `target\debug\kepler-backend.exe` and
`target\release\kepler-focus-svc.exe` failed while those binaries were running,
so isolated target dirs were used for build verification.

Additional fix: `services/kepler-focus-svc/src/service.rs` no longer joins the
pipe thread during SCM stop because it can block in `ConnectNamedPipe`.
`services/kepler-focus-svc/src/pipe.rs` now uses `std::fs::File` read/write on
the pipe handle instead of manual `ReadFile`/`WriteFile`; pipe smoke passed
after reinstall.
