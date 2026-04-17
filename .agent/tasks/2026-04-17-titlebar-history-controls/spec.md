# Titlebar History Controls

## Goal

Добавить в shared desktop titlebar унифицированные кнопки "назад/вперёд" со state-aware disabled-состояниями и подключить их к реальному router history в Vue desktop-приложениях.

## Scope

- `packages/kepler-visuals`
- `apps/dashboard`
- `apps/delphi/ts`
- `apps/eden/ts`
- Точечная синхронизация AGENTS.md для затронутых приложений

## Out of Scope

- Любые изменения маршрутов или структуры страниц

## Acceptance Criteria

- AC1: В `@kepler/visuals` есть shared titlebar history-control компонент с кнопками назад/вперёд.
- AC2: Кнопки имеют явный disabled-state и визуально отличаются в disabled-состоянии.
- AC3: Dashboard использует shared history controls и корректно отключает кнопки, когда history назад/вперёд недоступна.
- AC4: Delphi TS использует shared history controls и корректно отключает кнопки, когда history назад/вперёд недоступна.
- AC5: Eden использует shared history controls и корректно отключает кнопки, когда локальная история экранов/записей пуста.
- AC6: Изменение не ломает typecheck затронутых Vue приложений.
- AC7: Затронутые AGENTS.md отражают новый shared titlebar navigation contract.

## Verification Plan

- `tsc --noEmit -p apps/dashboard/tsconfig.json`
- `tsc --noEmit -p apps/delphi/ts/tsconfig.json`
- `tsc --noEmit -p apps/eden/ts/tsconfig.json`
- `git diff --stat`
