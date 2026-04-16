# Task Spec — Eden 100k typing stress benchmark baseline

## Goal
Добавить отдельный воспроизводимый stress benchmark для Eden на ~100k символов, снять текущий baseline и сохранить артефакты так, чтобы потом можно было честно сравнить метрики после следующих оптимизаций.

## Acceptance Criteria
- AC1: В репозитории есть отдельный запускаемый benchmark для Eden typing/open path на заметке ~100k+ символов.
- AC2: Benchmark сохраняет raw artifact с метриками baseline в `.agent/tasks/2026-04-15-eden-100k-stress-benchmark/artifacts/`.
- AC3: В baseline есть хотя бы: размер документа, open metric, normal typing metric, zen typing metric, long-task info.
- AC4: Запуск benchmark не ломает основной app build/test flow.
- AC5: Есть краткое evidence summary, чтобы потом можно было сравнить "до/после улучшательств".

## Constraints
- TipTap остаётся.
- Benchmark должен быть отдельным и переиспользуемым, а не одноразовым ручным прогоном.
- Артефакты должны сохраняться в task folder.
