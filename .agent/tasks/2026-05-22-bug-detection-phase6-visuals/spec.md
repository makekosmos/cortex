# 2026-05-22 bug-detection-phase6-visuals

## Context

Phase 6 из bug-detection roadmap. Phase 1-5 уже на месте; теперь нужно
расширить slate автоматических проверок:

1. **Component tests** (vitest browser) — у Eden уже есть pattern
   (`extensions/eden/vitest.config.ts` + `tests/components/*.spec.ts`).
   Delphi и Horologion — нет. Реплицируем минимум: smoke-моунт пары
   компонентов в реальном Chromium.

2. **Visual regression snapshots** — Playwright `toHaveScreenshot` для
   критичных UI-состояний (launcher initial, Eden journal). Дешёвая
   защита от CSS-регрессий, которые typecheck не ловит.

Цель — поднять plausible-bug-detection без полноценного покрытия:
любое будущее изменение в Delphi/Horologion компонентах хотя бы пройдёт
through-mount; визуальная регрессия будет видна как diff.

## Scope

В задаче:

- `extensions/delphi/vitest.config.ts` + `test:vue` script + 1-2 component
  spec'а (минимальное — `Skeleton.vue`, `InfoCard.vue`).
- `extensions/horologion/vitest.config.ts` + `test:vue` script + 1-2
  component spec'а (минимальное — pure-render компоненты вроде `format`
  обёртки или MentionMenu без deps).
- `tests/e2e/visual.spec.ts` — 3-5 `toHaveScreenshot` snapshots
  (launcher, Eden, опционально Settings).
- Baseline screenshots закоммичены в
  `tests/e2e/visual.spec.ts-snapshots/`.

Не в задаче:

- Полное covered покрытие всех компонентов.
- Cross-platform snapshots (Windows-only).
- Refactor существующих компонентов под testability.

## Acceptance Criteria

AC1. `extensions/delphi/vitest.config.ts` существует, моделирует
Eden pattern (browser mode, chromium, headless).

AC2. `extensions/delphi/tests/components/*.spec.ts` содержит >= 1 spec,
`bun run --cwd extensions/delphi test:vue` зелёный.

AC3. То же самое для horologion (AC1+AC2).

AC4. `tests/e2e/visual.spec.ts` содержит >= 3 `toHaveScreenshot` теста,
baseline сгенерирован (`--update-snapshots`) и закоммичен.

AC5. `bunx playwright test tests/e2e/visual.spec.ts` (без update-snapshots)
зелёный.

AC6. Pre-commit guards (`bunx oxfmt`, lefthook) проходят без обхода.

## Out of scope decisions

- Если конкретный component spec упирается в heavy deps (window.delphi,
  pinia store с реальным backend'ом) — пишем минимальный fixture, не
  тащим store. Аналогично Eden's CharCounter approach.
- Screenshot diff threshold: `maxDiffPixels: 200` (Windows font rendering
  меняется между обновлениями ОС, нужен небольшой запас).
