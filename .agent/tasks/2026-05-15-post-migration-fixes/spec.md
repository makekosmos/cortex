# Post-migration fixes — расширения работают так же как до Phase 4

## Контекст

После Phase 4 миграции Eden/Delphi/Horologion/Arrancador в Vue-extensions
наблюдаются регрессии: визуальные баги, не работающие функции, потерянные
интеграции. До миграции (как standalone Electron apps) приложения работали
корректно. Юзер ожидает **полную функциональную и визуальную parity**
с pre-Phase-4 состоянием.

## Acceptance criteria

### Functional (must)

- **AC1**: Делphi sidebar показывает ВСЕ items: «Входящие», «Сегодня»,
  «Журнал», «Корзина», группа «Проекты». Проверяется Playwright DOM query.
- **AC2**: Делphi показывает реальные задачи. В Today view видна задача
  `Деструктурирующее присваивание` (scheduled 2026-05-13). В Inbox пусто
  (no project-less, non-scheduled tasks) — корректное поведение.
- **AC3**: Horologion stopwatch start → счётчик идёт, кнопка «Стоп» виден
  → клик останавливает таймер → entry создан с правильным duration.
- **AC4**: Horologion pomodoro start → фокус-фаза идёт → можно paused/
  resume → автоматически завершается через configured duration → break
  фаза стартует автоматически.
- **AC5**: Console чистый без `Error: missing field 'id'` при normal
  operation (open launcher → open Делphi → open Horologion → invoke
  command bus).
- **AC6**: Command bus invoke из launcher (e.g. «Pomodoro 25 минут»)
  корректно стартует pomodoro в Horologion (cross-extension command).

### Build hygiene (should)

- **AC7**: `bun run --cwd shell build:js` clean без warnings:
  - Нет `INEFFECTIVE_DYNAMIC_IMPORT` (Делphi useQuickEntry.ts,
    shell extension-host.ts).
  - Нет `inlineDynamicImports option is deprecated`.
  - Нет `/fonts/zed-mono-extended.ttf didn't resolve` warning.

### Test infrastructure (must)

- **AC8**: Playwright e2e infrastructure установлена. Spec файлы в
  `tests/e2e/` (root) или в каждом extension. CI команда `bun run
  test:e2e`.
- **AC9**: **Test DB isolation** жёстко: все Playwright прогоны
  используют `%APPDATA%\Kosmos-test\ark.db` (или `.e2e/<spec>.db`),
  никогда `%APPDATA%\Kosmos\ark.db`. Backend поддерживает
  `KOSMOS_DATA_DIR` env override.
- **AC10**: AC1-AC6 проверяются автоматически в Playwright specs. PASS
  на CI без human input.

### Docs (must)

- **AC11**: `docs-site/concepts/test-isolation.md` обновлён с
  `KOSMOS_DATA_DIR` конвенцией. AGENTS.md/CLAUDE.md перегенерированы.
- **AC12**: Запреты «никогда не указывай тестам user DB path»
  продублирован в `docs-site/agents/forbidden.md`.

## Out of scope (отложить)

- Перенос JS/TS логики в Rust backend (архитектурное долго-runway,
  не блокирует AC1-AC10).
- Refactor Horologion / Делphi state management для Pinia consistency.
- Eden migration (он standalone до Phase 6).

## Approach

**Phase A — Test infrastructure (foundation):**
1. Playwright install + config. Test DB convention.
2. Backend `KOSMOS_DATA_DIR` env override.
3. Documentation refresh.

**Phase B — Quick fixes (independent):**
1. Build warnings (INEFFECTIVE_DYNAMIC_IMPORT, deprecated options).
2. `missing field 'id'` diagnostic + fix.

**Phase C — Functional regressions (validated by tests):**
1. Делphi sidebar (still empty after dedupe e5d3d18 — need DOM-level
   diagnostic via Playwright).
2. Horologion stopwatch/pomodoro state management.
3. Command bus cross-extension wiring.

Каждая фаза = коммиты, каждый AC проверяется в evidence.md.
