# Delphi Store Structured Clone Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-clipboard-flush-test-cleanup/desloppify-after.json`
- Score: `9`
- Findings: `201`
- Severity: `HIGH 57`, `MEDIUM 101`, `LOW 43`
- `JSON_DEEP_CLONE`: `1`

## Change

- Replaced `JSON.parse(JSON.stringify(todo))` in `products/delphi/src/store/todos.ts`.
- Added `cloneTodoForArk()` using `toRaw()` and `structuredClone()`.
- Added a typed fallback that copies the mutable nested arrays/objects used by `TodoItem`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-delphi-structured-clone-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `200`
- Severity: `HIGH 57`, `MEDIUM 100`, `LOW 43`
- `JSON_DEEP_CLONE`: `0`

## Checks

- `rtk grep "Ð|Ñ|Рџ|�|Â|â€|Ã|JSON\.parse\(JSON\.stringify" products/delphi/src/store/todos.ts`
- `rtk test bun run --cwd products/delphi test:vue` failed on pre-existing class assertions:
  - `expected 'info-card' to match /rounded-xl/`
  - `expected 'animate-pulse rounded-[var(--radius-i…' to match /rounded-md/`
- `rtk err bunx vue-tsc --noEmit -p products/delphi/tsconfig.json` failed because `bun-types` is missing from the current type environment.
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-delphi-clone.json"`
