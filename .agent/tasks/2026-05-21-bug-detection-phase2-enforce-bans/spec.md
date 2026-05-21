# 2026-05-21 bug-detection-phase2-enforce-bans

## Context

`CLAUDE.md` + `docs-site/agents/forbidden.md` содержат большой список ❌
запретов, которые сейчас ловятся только в head'ах ревьюеров. `bun run
ark:guard:writes` покрывает прямые SQL writes (одну часть), но не покрывает
runtime-инварианты вроде `app.getPath('userData')` вне `instance.ts`,
`KOSMOS_DATA_DIR` указывающий на `%APPDATA%` в e2e, `console.log` в production
shell code, `Mutex::lock().unwrap()` в prod Rust, и т.п.

Эта Phase 2 из bug-detection roadmap (см. чат 2026-05-21). Phase 1
(determinism layer) уже сделан (см. соседний `2026-05-21-bug-detection-phase1-determinism/`).

Технологический стек **исправлен**: используем **oxlint**, а не ESLint
(пользователь обратил внимание; oxlint был раньше в репо, удалён в одной из
миграций apps→extensions). Cargo clippy через `[workspace.lints]`. Custom
runtime checks остаются в `scripts/check-ark-write-boundaries.mjs`.

## Scope

В задаче:

- `bun add -D -E oxlint` в root `package.json`.
- `.oxlintrc.json` в корне с:
  - `categories.correctness=error`, `categories.suspicious=warn`,
  - точечный `no-restricted-syntax` для `app.getPath('userData')`
    (whitelist `shell/electron/instance.ts` через `overrides`),
  - `no-console` в `shell/electron/**` (с whitelist'ом существующих
    `console.error` если их слишком много — это будет видно после первого
    прогона; если меньше 20, починим; больше — выставим `warn`).
- `Cargo.toml` `[workspace.lints]`:
  - `clippy.unwrap_used = "warn"` (не deny — слишком много existing usages,
    которые требуют отдельной чистки; см. forbidden.md упоминание про
    poison recovery),
  - `clippy.panic = "warn"`,
  - `rust.unused_must_use = "deny"`.
- Расширение `scripts/check-ark-write-boundaries.mjs`:
  - Scan `app.getPath('userData')` / `app.getPath("userData")` вне
    `shell/electron/instance.ts` — fail.
  - Scan `KOSMOS_DATA_DIR.*APPDATA|%APPDATA%.*KOSMOS_DATA_DIR` в
    `tests/e2e/**.ts` (вне `helpers/launch.ts`) — fail.
  - Scan `path.join.*['"]Kosmos['"]|['"]Kepler['"]` в `shell/electron/**.ts`
    вне `instance.ts` / `data-dir.ts` — fail.
  - Все scan'ы возвращают массив findings в общий exit code; одна команда
    `bun run ark:guard:writes` — все проверки.
- `lefthook.yml`:
  - `pre-commit::oxlint` — `bunx oxlint {staged_files}` с glob по
    `*.{ts,tsx,vue,mjs,cjs,js,jsx}`.
  - `pre-push::clippy` — `cargo clippy --workspace --all-targets -- -D warnings`.

Не в задаче:

- Чистка существующих `unwrap_used` warning'ов до zero. Включаем `warn`,
  не `deny` — отдельная гигиеническая задача потом.
- CI workflow (Phase 3 — отдельный proof loop).
- Замена legacy hand-rolled checks на oxlint полностью. Параллельная
  жизнь: oxlint для статики, `ark:guard:writes` для runtime/path-aware.
- JS Plugins API (alpha в oxlint) — пока не используем. Только
  `no-restricted-syntax`.
- `console.log` в renderer code (`shell/src/**`, `extensions/*/src/**`) —
  оставляем (там много legacy debug-логирования; чистится отдельно).

## Acceptance Criteria

AC1. `bun add -D -E oxlint` успешно поставлен. `bunx oxlint --version`
возвращает версию.

AC2. `.oxlintrc.json` валидируется через `bunx oxlint --rules` без syntax
errors. Точечные ban-rules срабатывают на синтетическом сэмпле
(добавить `app.getPath('userData')` в любой файл shell/electron вне
instance.ts → oxlint выдаёт error).

AC3. `bunx oxlint .` на чистом working tree прогоняется без новых error'ов
(только warning'и; кол-во warning'ов задокументировано в `evidence.md`).

AC4. `cargo clippy --workspace --all-targets -- -D warnings` — green или
с задокументированным точным списком warning'ов (если warning'ов
больше 0 — `unwrap_used` оставляем `warn` и в clippy CLI не передаём
`-D warnings`, проверка работает на `cargo clippy --workspace
     --all-targets` без флагов).

AC5. `bun run ark:guard:writes` зелёный, новые scan'ы покрывают: - `app.getPath('userData')` outside `instance.ts` → fail (проверить
синтетической вставкой), - `KOSMOS_DATA_DIR=...APPDATA...` в `tests/e2e/*` outside
`helpers/launch.ts` → fail (синтетическая вставка), - `path.join(*, "Kosmos"/"Kepler", ...)` outside instance/data-dir →
fail (синтетика).

AC6. `lefthook.yml` содержит oxlint в pre-commit и clippy в pre-push.
`lefthook run pre-commit --files shell/electron/main.ts` отрабатывает
без regression'ов (то есть проходит для текущего clean tree).

AC7. Phase 1 e2e suite (`bun run test:e2e`) — не сломан. То есть
добавление oxlint / clippy / расширение ark:guard не нарушает
deterministic behavior. Smoke прогон достаточен (один-два spec'а).

## Verification commands

- `bunx oxlint --version` — AC1.
- `bunx oxlint .` — AC2, AC3.
- `cargo clippy --workspace --all-targets` — AC4.
- `bun run ark:guard:writes` — AC5.
- Синтетические нарушения (см. evidence.md «как проверял») — AC2, AC5.
- `lefthook run pre-commit --files <file>` — AC6.
- `bunx playwright test tests/e2e/eden.spec.ts` — AC7 (один spec для smoke).

## Out of scope decisions

- Если oxlint выдаёт > 50 warning'ов в renderer code — оставляем как baseline,
  чистится в отдельной задаче.
- Если есть `unwrap_used` в **prod** path Rust (вне `#[cfg(test)]`) больше
  10 — записываем список в evidence.md как technical debt, не блокируем
  Phase 2.
- Custom `no-restricted-syntax` правила для `await props.onSave` без
  try/catch — откладываются до Phase 6 (когда у нас будет нужда в нескольких
  custom-rule паттернах разом, проще будет включить JS Plugins API стабильно).
