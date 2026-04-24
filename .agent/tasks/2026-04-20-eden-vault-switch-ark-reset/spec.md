# Eden Vault Switch Ark Reset

## Summary

Исправить смену vault в `Eden`: после выбора новой папки `Eden` должен сразу переключаться на соответствующую Ark DB, а не продолжать читать старую базу до перезапуска приложения.

## Acceptance Criteria

- AC1: `setVaultPath()` в `apps/eden/ts/main/store.ts` сбрасывает Ark client после обновления shared selected space.
- AC2: После смены vault следующий запрос к Ark использует новый `selected-space` path resolution.
- AC3: Fallback-поведение без shared selected space не ломается.
- AC4: Сборка `apps/eden/ts` проходит.
