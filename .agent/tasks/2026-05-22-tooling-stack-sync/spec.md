# 2026-05-22 tooling-stack-sync

## Context

После Phase 2 bug-detection стало видно что доки описывают tooling stack
который не реализован: `bun run lint`, `bun run format` упоминаются в
`docs-site/guide/tooling.md`, но скриптов нет в корневом `package.json`;
oxfmt в root devDeps но никогда не вызывался; `.oxfmtrc.json`/`.oxlintrc.json`
отсутствовали; lefthook не содержит ни одного из этих guards.

История: Phase A migration (apps→extensions, commit `c097f237`) выкосила
`apps/delphi/ts/.oxlintrc.json` + `apps/delphi/ts/.oxfmtrc.json` и не
восстановила глобально.

## Scope

В задаче:

- `bunx oxfmt --init` + `.oxfmtrc.json` с ignorePatterns.
- root `package.json` scripts: `lint`, `format`, `format:check`.
- Удалить локальный `oxfmt@^0.7.0` из `packages/visuals/devDependencies`
  (workspace-level oxfmt@^0.43.0 покрывает).
- Массовый `bunx oxfmt .` apply — 1106/1430 файлов отформатировать в
  одном commit'е.
- Добавить `oxfmt --check` в `lefthook.yml::pre-commit`.
- Обновить `docs-site/guide/tooling.md`: убрать ложные утверждения,
  привести в соответствие с состоянием после Phase 2 + Phase 2.5.

Не в задаче:

- Чистка существующих 16 oxlint warnings.
- Custom oxlint rules / JS plugins.
- CI workflow (Phase 3).
- Битые ссылки в других docs-файлах (`/memory` в kepler-roadmap.md).

## Acceptance Criteria

AC1. `bun run lint`, `bun run format`, `bun run format:check` — все три
скрипта существуют и работают (exit 0 на clean tree).
AC2. `bunx oxfmt --check .` → All matched files use the correct format.
AC3. Massive format commit не сломал ничего: - `bun run --cwd shell typecheck` green, - `bunx oxlint .` 0 errors (warnings same as before), - `bun run ark:guard:writes` passed, - smoke `bunx playwright test tests/e2e/eden.spec.ts` ≥ 9/9 passed.
AC4. `lefthook.yml` содержит oxfmt --check в pre-commit;
`bunx lefthook validate` → All good.
AC5. `docs-site/guide/tooling.md` обновлён под реальность; `bun run docs:sync`
зелёный; `bun run docs:check` зелёный _по этому файлу_ (pre-existing
`/memory` link в другом файле — known issue).
AC6. `packages/visuals/package.json` не содержит локальной oxfmt dep.
