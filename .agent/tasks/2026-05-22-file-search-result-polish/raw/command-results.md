# Command Results

## Backend Focus

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index
```

```text
5 passed; 0 failed
```

## Backend Library

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib
```

```text
156 passed; 0 failed
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

## ARK Guards

```powershell
bun run ark:guard:writes
bun run ark:smoke
```

```text
ARK write boundary guard passed.
ARK smoke matrix passed.
```

The first smoke attempt raced the temporary renderer visual-check process and
could not replace the workspace `ark-core-rpc.exe`. After stopping that
workspace Vite/Electron process, the smoke command was rerun and passed.

## Renderer Smoke

The local Playwright renderer smoke mocked `file_index.search` with:

- one long TypeScript path
- one blank `name` with path ending in `fallback-file.json`

Observed rows:

```json
[
  {
    "title": "json-renderer-with-a-very-long-path.ts",
    "pathClientWidth": 500,
    "pathScrollWidth": 666
  },
  {
    "title": "fallback-file.json",
    "pathClientWidth": 288,
    "pathScrollWidth": 288
  }
]
```
