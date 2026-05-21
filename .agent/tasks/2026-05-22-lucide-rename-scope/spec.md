# Lucide rename scope: `lucide-vue-next` → `@lucide/vue`

**Дата:** 2026-05-22
**Тип:** chore / deps maintenance
**Substantial:** mechanical rename, без изменения функционала.

## Контекст

Lucide объявил deprecation старого scope `lucide-vue-next` в пользу нового
`@lucide/vue`. Оба пакета пока работают, но в течение 6-12 месяцев старый
перестанет получать обновления. Делаем upfront миграцию, пока diff
тривиальный.

## Scope

7 workspace'ов с зависимостью `lucide-vue-next@^1.0.0`:

- `shell/`
- `extensions/horologion/`
- `extensions/arrancador/`
- `extensions/delphi/`
- `extensions/eden/`
- `packages/visuals/`
- `site/`

Плюс `packages/visuals/package.json::peerDependencies["lucide-vue-next"]`.

Импорты вида `import { Play, Pause } from "lucide-vue-next"` в Vue / TS файлах.

## Acceptance Criteria

- AC1: ни в одном `package.json` нет `lucide-vue-next` (в deps / peerDeps / devDeps).
- AC2: `bun run --cwd shell build:js` зелёный.
- AC3: `bun run --cwd shell typecheck` зелёный.
- AC4: `bunx oxlint .` без errors (warnings допустимы — pre-existing).
- AC5: `bunx oxfmt --check .` clean.
- AC6: `bunx playwright test tests/e2e/eden.spec.ts` — 9/9 passed.
- AC7: grep `lucide-vue-next` в активном коде (без `legacy/`, `.agent/tasks/raw/`, `.vitepress/dist/`) — 0 matches.

## Не делать

- Не трогать функционал — это чисто rename имени пакета.
- Не обновлять версии других зависимостей.
- Не править `legacy/dashboard-extension/` (frozen).
- Не переписывать историю в `.agent/tasks/*/raw/` (захардкоженные diff'ы).
