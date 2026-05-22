# Command Results

## Focused File Index Tests

Command:

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index
```

Result:

```text
3 passed; 0 failed
```

## Kepler Backend Library Tests

Command:

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib
```

Result:

```text
154 passed; 0 failed
```

## Shell Typecheck

Command:

```powershell
bun run --cwd shell typecheck
```

Result:

```text
tsc --noEmit
exit code 0
```

## Shell Build

Command:

```powershell
bun run --cwd shell build:js
```

Result:

```text
shell renderer/main/preload build and extension build pipeline completed
exit code 0
```

## ARK Write Guard

Command:

```powershell
bun run ark:guard:writes
```

Result:

```text
ARK write boundary guard passed.
```

## ARK Smoke

Command:

```powershell
bun run ark:smoke
```

Result:

```text
ARK smoke matrix passed.
```

## Fresh Verification After Evidence

After the live watcher and prefix-safe subtree removal were added, the verifier
pass reran:

- `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib`
- `bun run --cwd shell typecheck`
- `bun run --cwd shell build:js`
- `bun run ark:guard:writes`
- `bun run ark:smoke`

All five commands exited with code `0`.
