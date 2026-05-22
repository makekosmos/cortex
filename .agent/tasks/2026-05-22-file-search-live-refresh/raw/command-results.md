# Command Results

## File Search Focus

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

## Renderer Smoke

The local renderer smoke mocked `file_index.search` so the first request
returned no rows and the next refresh returned one long file row without any
additional typing or launcher show event.

```json
{
  "firstPassRows": 0,
  "calls": 2,
  "title": "some-really-important-file-title-that-should-win-over-the-path.json",
  "titleClientWidth": 462,
  "titleScrollWidth": 462,
  "pathClientWidth": 237,
  "pathScrollWidth": 525,
  "kindVisible": true
}
```
