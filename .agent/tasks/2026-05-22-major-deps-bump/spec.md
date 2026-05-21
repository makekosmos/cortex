# 2026-05-22 major-deps-bump

## Context

После Phase 2.5 (tooling stack sync) сделан полный аудит зависимостей. 5 major
bumps признаны актуальными к выполнению одной волной. `@types/node 24 → 25`
**не делается** — `@types/node` должен соответствовать реальному runtime'у
(bun 1.3, Electron 42 встроенная Node 22). Поднимать впереди реального Node
поднимает риск использовать API'ы, которых нет в runtime.

## Scope

В задаче:

- vue-router 4.x → 5.0.7 (no-op по research для нашего composition-API).
- uuid 13.0.2 → 14.0.0 в `extensions/eden` (named imports не тронуты).
- lucide-vue-next 0.548 → 1.0.0 (brand icons grep — clean).
- electron 41.6 → 42.2 (native deps у нас нет → rebuild не нужен).
- typescript 5.8.3 → 6.0.3 (последним; самый рисковый).

Не в задаче:

- Переход lucide-vue-next → `@lucide/vue` (новый scope) — отдельная задача.
- Чистка 16 oxlint warnings / 27 cargo unwrap_used — отдельный hygiene.
- Unlisted `lucide-vue-next` / `vue-router` в arrancador/delphi/eden
  package.json — отдельная hi.fix.
- Patch/minor bumps — отдельно через `bun update`.

## Acceptance Criteria

AC1. Все 5 bump'ов закоммичены отдельными commit'ами с per-bump verify.

AC2. После всех bump'ов: - `bun run --cwd shell typecheck` → green, - `bun run --cwd packages/ark typecheck` → green, - `bun run --cwd shell build:js` → green, - `bunx oxlint .` → 0 errors (warnings same baseline), - `bunx oxfmt --check .` → all clean, - `bun run ark:guard:writes` → passed, - `cargo clippy --workspace --all-targets` → 0 errors, - `bunx playwright test tests/e2e/eden.spec.ts` → 9/9 passed.

AC3. TypeScript 6 deprecations не блокируют — `"ignoreDeprecations": "6.0"`
добавлен в 3 tsconfig (shell, delphi, site) для `baseUrl`. Долгосрочный
fix (relative paths) — отдельная задача до TS 7.

AC4. `@types/node` остаётся на 24.x — обоснование задокументировано.
