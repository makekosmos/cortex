# Command Results

## Focused File Search

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index
```

```text
7 passed; 0 failed
```

The focused suite covers File Search FTS substring search and NTFS parent-FRN
path reconstruction/noisy-folder filtering in addition to the existing File
Search tests.

## Backend Library

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib
```

```text
158 passed; 0 failed
```

## Shell

```powershell
bun run --cwd shell typecheck
bun run --cwd shell build:js
```

```text
typecheck passed
renderer/main/preload and extension build pipeline completed
```

## Renderer Pending Smoke

An isolated headless Kepler run with `KOSMOS_TEST_MODE=1`,
`KOSMOS_HEADLESS=1`, and `KEPLER_FILE_INDEX_ROOTS=<shell/src>` filled the
launcher with `LauncherView`.

The Playwright smoke awaited the text `Ищем файлы...` while `file_index.search`
was in flight and then awaited a `.file-row` result. The command exited `0`.

## ARK Guards

```powershell
bun run ark:guard:writes
$env:CARGO_TARGET_DIR='D:\Personal\Hobby\Coding\kosmos\target-ark-smoke-file-search'; bun run ark:smoke
```

```text
ARK write boundary guard passed.
ARK smoke matrix passed.
```

The first plain `bun run ark:smoke` attempt failed before tests because the
running dev Kepler held `target\debug\ark-core-rpc.exe`; the isolated target
directory rerun completed successfully without stopping the user's dev process.
