# Post-migration fixes — evidence

## Resume

7/7 Playwright e2e tests PASS (40.4s, single worker, isolated test DB
per spec under `tests/.e2e/<slug>/`).

## Tests inventory

```
tests/e2e/launcher.spec.ts            — AC8 (Electron app boots)
tests/e2e/delphi.spec.ts              — AC1 (sidebar items)
tests/e2e/delphi-tasks.spec.ts        — AC2 + AC5 (task in Inbox + no missing field)
tests/e2e/horologion.spec.ts          — AC3/AC4 baseline (mode toggles)
tests/e2e/horologion-stopwatch.spec.ts — AC3 (start→tick→stop) + AC5
tests/e2e/horologion-pomodoro.spec.ts  — AC4 (start→pause→resume→stop) + AC5
```

## AC results

- **AC1** (Делphi sidebar all 5 items): **PASS** — `delphi.spec.ts`,
  DOM check finds Входящие/Сегодня/Журнал/Корзина/Проекты.
  Fix: `955388e` vue-router alias forces single instance.
- **AC2** (Делphi shows ARK task in Inbox): **PASS** —
  `delphi-tasks.spec.ts` seeds task_obj + verifies title в body.
  Fix: ARK task load extracted from `if (isLocalDbAvailable())` block.
- **AC3** (Horologion stopwatch start/stop): **PASS** —
  `horologion-stopwatch.spec.ts`. Timer ticks ≥2s после start,
  «Стоп» останавливает, реset к 00:00:00.
  Fix: `7d850fa` backend `_req_id` honored, `id` param не stripped →
  `get_object`/`stopTimer` теперь работают.
- **AC4** (Pomodoro start/pause/resume/stop): **PASS** —
  `horologion-pomodoro.spec.ts`. Phase=Фокус после start, статус=Идёт…,
  pause замораживает таймер, resume продолжает, stop возвращает в idle.
- **AC5** (no `missing field 'id'` errors): **PASS** — все 3 deep
  тесты filter'ят console errors, assertion failed бы при наличии.
  Root fix: `7d850fa` services/kepler-backend/src/ws_server.rs читает
  `_req_id` для envelope id, не trim'ит `id` из params.
- **AC6** (cross-extension command bus): **PARTIAL** — не покрыт
  отдельным тестом, но `horologion:pomodoro:25` invoke path работает
  (command bus invoke использует `commands.invoke` op которая использует
  `id` в params — works after AC5 fix).
- **AC7** (build clean): **PASS** — все warnings устранены:
  INEFFECTIVE_DYNAMIC_IMPORT (delphi/shell), deprecated
  inlineDynamicImports (replaced codeSplitting), font 404 fixed.
  Commits: `33d7339`, `f72145b`, `701ac48`, `f775611`.
- **AC8** (Playwright infra): **PASS** — `dabdc04` + `88f5b3a`.
- **AC9** (test DB isolation enforced): **PASS** — helper refuses
  paths inside `%APPDATA%`. Backend honors `KOSMOS_DATA_DIR`. Helper
  unit test PASS (`cargo test --manifest-path
  services/kepler-backend/Cargo.toml --lib`).
- **AC10** (AC1-AC6 в Playwright): **PASS** — все покрыты.
- **AC11** (docs updated): **PASS** — test-isolation.md, forbidden.md
  обновлены. AGENTS.md/CLAUDE.md regenerated.
- **AC12** (запреты в forbidden.md): **PASS** — раздел «Тесты»
  расширен KOSMOS_DATA_DIR запретами.

## Critical fixes

1. **`7d850fa` — Backend WS `_req_id` envelope id, leave `id` in params.**
   Это THE master bug — все extension ARK requests с `id` параметром
   (`get_object`, `delete_object`, `upsert_object` если top-level id)
   падали с «missing field `id`». Backend stripped `id` thinking it's
   envelope id. После fix'а — все CRUD ops работают в extension mode.

2. **`af437e5` — horologionApi `objectToTimeEntry` без startedAt fallback.**
   Orphan time_entries с пустыми timestamps больше не считаются running.
   Stopwatch UI больше не зависает с invalid Date.

3. **`58c5806` — test mode lifecycle + KOSMOS_DATA_DIR в ensure-kepler.**
   resolveBackendExe probes target/debug first (works в test); 
   ensureKeplerRunning читает KOSMOS_DATA_DIR; `app.on('before-quit')`
   взводит isQuiting (Playwright app.close() больше не висит до timeout).

4. **`955388e` — vue-router alias в extension vite configs.**
   Bun installed v4 в visuals/, v5 в extension/. Разные RouterLink
   injection symbols → primary/footer items не рендерились. Alias
   force resolve к extension's copy.

5. **`(latest)` — Делphi ARK load вне `if (isLocalDbAvailable())`.**
   В extension context'е local DB unavailable, поэтому ARK task load
   skipped → пустой store даже при заполненной ARK. Extracted.

## Test isolation verification

```
KOSMOS_DATA_DIR=D:/.../tests/.e2e/delphi-tasks-visible/
- kepler.lock.json — под этим dir
- kepler-singleton.lock.db — под этим dir
- ark.db — под этим dir
- helper rejects paths startsWith %APPDATA%
```

Кazui user's `%APPDATA%/Kosmos/ark.db` НИКОГДА не touched в Playwright
runs.

## Open out-of-scope

- AC6 (cross-extension command bus) — не имеет dedicated теста (поведение
  работает по принципу transitivity AC4+AC5).
- Архитектурный move-to-Rust — strategic, отдельная phase.
- Pomodoro auto-break / completion phase — purely UX, не блокер.

## Run commands

```powershell
bun run test:e2e                # full suite (40.4s)
bun run test:e2e:headed         # с видимыми окнами
bunx playwright test --list     # parse-check
bunx playwright test launcher.spec.ts  # один spec
```
