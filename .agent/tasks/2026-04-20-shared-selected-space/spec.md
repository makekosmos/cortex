# Shared Selected Space

## Summary

Добавить общий маркер выбранного space/личной базы в `appData/Kepler`, чтобы `Eden`, `Delphi` и `Arrancador` смотрели на один и тот же выбранный Ark DB. `Eden` дополнительно должен уметь записывать в этот маркер выбранный `vaultPath` для личного пространства.

## Acceptance Criteria

- AC1: В репозитории есть общий helper для чтения/записи shared selected space в `appData/Kepler/selected-space.json`.
- AC2: `Eden` при выборе vault сохраняет shared selected space и использует его для выбора пути к Ark DB вместо жесткого `appData/Kepler/ark.db`.
- AC3: `Arrancador` при старте читает shared selected space и использует соответствующий Ark DB path.
- AC4: `Delphi Electron` читает shared selected space на старте, переключает sidecar на выбранный `spaceId`, а его `space:getActive` / `space:setActive` синхронизируются с shared selected space.
- AC5: Если shared selected space отсутствует, все приложения сохраняют старое fallback-поведение и продолжают работать.
- AC6: Затронутый UI/тексты не содержат mojibake.

## Notes

- Для личного пространства `Eden` должен создавать стабильный `spaceCode` и `spaceId` из выбранного `vaultPath`, чтобы `Delphi` мог отображать и использовать тот же selected space.
- В рамках этой задачи не требуется мигрировать локальные app-specific базы вроде `arrancador.db`; синхронизируется именно выбор Ark DB.
