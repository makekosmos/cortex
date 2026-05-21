# 2026-05-22 bug-detection-phase3-ci

## Context

Phase 3 из bug-detection roadmap. У репо **нет CI** — `find .github` пусто.
Каждая регрессия ловится либо локально, либо в production crash reports.

После Phase 1 (deterministic e2e), Phase 2 (machine-enforced bans),
Phase 2.5 (tooling sync), Phase 2.6 (major bumps) — у нас есть:

- Stable green test suite (eden.spec.ts 9/9 за 50s; full suite 79 passed,
  2 pre-existing failures, 10m36s).
- Lint/format clean baseline (oxlint 0 errors, oxfmt all clean).
- Guards: `ark:guard:writes` с 4 invariant scan'ами.
- `cargo clippy --workspace --all-targets` зелёный.

Готово стандартное pipeline на GitHub Actions.

## Стратегия cost/benefit

В обсуждении плана пользователь явно отметил: Windows runner на каждый PR
дорогой (2× minute multiplier + ~10 мин cold cargo build × 4 shard = до
80 минут на PR). Стратегия:

- **PR (Ubuntu, fast):** guards + typecheck + clippy + cargo test + unit-ts.
  Цель — <5 минут на PR.
- **Nightly (Windows):** full Playwright e2e. Цель — ловить регрессии до
  следующего рабочего дня.

E2e на PR не запускаем (Windows-only requirement + cost).

## Scope

В задаче:

- `.github/workflows/ci.yml`:
  - `guards` (Ubuntu): `bun run ark:guard:writes`, `bunx oxlint .`,
    `bunx oxfmt --check .`, `bun run docs:check`.
  - `typecheck` (Ubuntu): `bun run --cwd shell typecheck`,
    `bun run --cwd packages/ark typecheck`.
  - `rust` (Ubuntu): `cargo clippy --workspace --all-targets`,
    `cargo nextest run --workspace --lib`.
  - `unit-ts` (Ubuntu): unit тесты per workspace (packages/ark, eden,
    delphi, horologion). bun test + vitest where applicable.

- `.github/workflows/nightly.yml`:
  - Windows runner, schedule cron midnight UTC.
  - Build backend + build:js, run full `bun run test:e2e`.
  - Upload trace.zip on failure (playwright уже configured).

- `.github/dependabot.yml`:
  - npm: weekly.
  - cargo: weekly.
  - github-actions: monthly.

- Cache: `actions/cache` для node_modules (через bun lockfile hash),
  `Swatinem/rust-cache` для target/.

Не в задаче:

- Branch protection rules на main (GitHub UI, не workflow).
- Build/publish flow (electron-builder, ext:publish) — отдельная задача
  release pipeline.
- macOS / Linux build/runtime — Windows-only по `forbidden.md`.
- Custom GitHub App с auto-merge — overkill.

## Acceptance Criteria

AC1. `.github/workflows/ci.yml` существует, валидный YAML, 4 jobs:
guards, typecheck, rust, unit-ts. Все на Ubuntu.

AC2. PR-симуляция: `act -j guards`, `act -j typecheck` локально (если
`act` доступен) ИЛИ push в test branch → PR → CI green.
Если ни то ни другое — задокументировать ручной dry-run каждого
`run` блока локально перед commit'ом.

AC3. `.github/workflows/nightly.yml` существует, cron schedule, Windows
runner, full e2e + artifact upload.

AC4. `.github/dependabot.yml` существует с npm/cargo/github-actions.

AC5. `bun run docs:check` зелёный после добавления workflow файлов
(не должно быть упоминаний несуществующих путей в новых .md).

AC6. Локально все steps из CI прогоняются зелёно (за исключением e2e
который только в nightly).

## Out of scope decisions

- Тесты на Mac/Linux не делаем — Windows-only product.
- `--frozen-lockfile` в CI: да, чтобы lockfile mismatch не привёл к
  silent dep bump.
- Cargo cache на Windows historically нестабильный — но для nightly это
  OK, не блокирует PR.
- Если cargo clippy/test займут больше 10 минут — оптимизация отдельная
  задача (cargo-pgo, split jobs).
