# 2026-06-01 — usage playtime precision

## Контекст

Пользователь заметил, что Dashboard показывает около 2 часов The Witcher 3,
хотя реальное playtime ближе к 10 часам. Два read-only расследования
подтвердили: Dashboard показывает то, что есть в ARK DB, но текущий
usage-tracker считает только foreground-window activity через Win32
`GetForegroundWindow`, а не runtime игрового процесса.

Дополнительный риск: `usage_tracker` запускается как background task внутри
backend и может завершиться навсегда после одной ошибки capture/write.

## Acceptance Criteria

**AC1.** `usage_sessions` хранит отдельное поле runtime/playtime, которое
увеличивается пока ранее обнаруженный процесс жив, даже если он не foreground.

**AC2.** Foreground/idle поля остаются совместимыми: они продолжают описывать
только активное окно и не подменяются process runtime.

**AC3.** Existing DB мигрирует additive-only: старые строки получают разумный
`runtime_ms` backfill из уже известных `foreground_ms + idle_ms`; destructive
migration отсутствует.

**AC4.** `get_usage_analytics` и Dashboard показывают суммарное время из
runtime/playtime, а foreground остаётся отдельной диагностической колонкой.

**AC5.** Одна ошибка Win32 capture или ARK write не убивает usage tracker
навсегда; loop продолжает следующие тики и логирует ошибку.

**AC6.** Регрессия покрыта автоматическими тестами на schema/analytics и
tracker accumulation/recovery logic.

**AC7.** Перед сдачей пройдены релевантные проверки: Rust tests для
ark-core/kepler-backend usage paths, shell typecheck/build, `ark:guard:writes`,
`docs:check`.
