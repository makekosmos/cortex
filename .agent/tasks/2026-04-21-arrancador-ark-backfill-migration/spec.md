# Arrancador Ark Backfill And Migration

## Summary

Исправить ситуацию, когда уже существующие игры в локальной базе `Arrancador` не появляются в `Eden`, потому что они никогда не были засинканы в Ark. Дополнительно дать явную возможность мигрировать `game_obj` из одной Ark DB в другую.

## Acceptance Criteria

- AC1: `Arrancador` умеет досинкать уже существующие локальные игры в текущую Ark DB, даже если они были добавлены раньше.
- AC2: При старте runtime в `Arrancador` запускается best-effort backfill уже существующих игр в Ark.
- AC3: Есть явный backend/API метод ручного досинка игр в Ark.
- AC4: Есть явный backend/API метод миграции `game_obj` из указанной Ark DB в текущую Ark DB.
- AC5: Есть CLI/script способ выполнить миграцию `game_obj` из одной Ark DB в текущую.
- AC6: Сборка/типизация затронутых TypeScript частей проходит.
