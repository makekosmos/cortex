# Object Type Icon Color Picker

## Summary

Переделать выбор иконки/цвета в редакторе типа объекта в Eden:

- убрать фоновую подложку у иконок в identity hero/trigger,
- заменить текущие `select` и `color input` на визуальный picker,
- picker должен открываться по нажатию на иконку и содержать отдельный выбор иконки и цвета.

## Acceptance Criteria

- AC1: В `ObjectTypeIdentitySection.vue` у основной иконки типа нет фоновой плитки/подложки.
- AC2: Вместо `select` для `icon` и текстового/color input для `color` есть popup/panel picker, открываемый по нажатию на иконку.
- AC3: Picker содержит два отдельных блока: выбор иконки и выбор цвета.
- AC4: Выбранные иконка и цвет сразу отражаются в preview текущего типа.
- AC5: Сборка `apps/eden/ts` проходит.
