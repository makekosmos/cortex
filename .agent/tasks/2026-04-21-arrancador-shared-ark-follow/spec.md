# Arrancador Follows Shared Ark DB

## Summary

Исправить `Arrancador`, чтобы он без рестарта подхватывал текущий общий `selected space`, который выбирается в `Eden`, и работал с тем же `ark.db`.

## Acceptance Criteria

- AC1: `Arrancador` определяет текущий целевой `ark.db` из общего `selected-space.json`, а не держит устаревший путь до рестарта.
- AC2: После смены папки/vault в `Eden` Ark-зависимые операции `Arrancador` используют новый `ark.db` без перезапуска приложения.
- AC3: Исправление покрыто регрессионной проверкой на смену целевого `ark.db`.
- AC4: Проверки на затронутый код проходят, а evidence сохранен в `.agent/tasks/2026-04-21-arrancador-shared-ark-follow/`.
