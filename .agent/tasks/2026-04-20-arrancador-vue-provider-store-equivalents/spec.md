# Task: Arrancador Vue equivalents for provider/store/hooks layer

## Original Task

User request: "Ты работаешь не один в кодовой базе, не откатывай чужие изменения и подстраивайся под них. Твоя зона ответственности: apps/arrancador/src/store/_, apps/arrancador/src/providers._, apps/arrancador/src/components/theme-provider._, apps/arrancador/src/components/language-provider._, apps/arrancador/src/components/ToastProvider.\*, apps/arrancador/src/hooks/useSettingsState.ts, apps/arrancador/src/hooks/useGameStatus.ts, apps/arrancador/src/hooks/useDropZone.ts, apps/arrancador/src/hooks/use-mobile.ts. Задача: спроектировать и по возможности реализовать Vue composables/plugins/store equivalents для React provider/hooks слоя, сохраняя API контракты с lib/api.ts и lib/browser.ts. В конце перечисли измененные файлы."

## Summary

Design and, where safely feasible, implement an isolated Vue Composition API layer for `apps/arrancador` that mirrors the current React provider/store/hooks behavior. The Vue layer must preserve the existing renderer contracts with `src/lib/api.ts` and `src/lib/browser.ts` and must not disrupt the current React runtime.

## Component / Module Map

- `src/vue/providers/*`: app-wide plugin installation and provider-equivalent wiring for theme, language, and toast services.
- `src/vue/composables/*`: Vue equivalents for `useSettingsState`, `useGameStatus`, `useDropZone`, and `useIsMobile`.
- `src/vue/store/*`: shared games state/actions exposed as a Pinia-style or composable store equivalent for `GamesContext`.
- `src/vue/components/*`: only if a provider-equivalent requires a render surface, such as toast viewport mounting.

## Acceptance Criteria

- AC1: A frozen design exists for Vue equivalents of the current React-owned files in scope, including provider wiring, shared games state, and the listed hooks.
- AC2: The implemented Vue layer is isolated from the current React runtime so existing React entrypoints and behavior are not broken.
- AC3: Vue equivalents preserve the external behavioral contracts of the current layer where they touch `src/lib/api.ts` and `src/lib/browser.ts`; no API signature changes are introduced there.
- AC4: The Vue equivalents cover the current responsibilities of theme state, language state/translation lookup, toast notification dispatch/listening, games state/actions, settings state, game status polling, drag-and-drop integration, and mobile breakpoint detection.
- AC5: Verification artifacts under `.agent/tasks/2026-04-20-arrancador-vue-provider-store-equivalents/` record what was checked, what passed/failed, and the current limitations or follow-up gaps.

## Constraints

- Do not revert or overwrite unrelated user changes.
- Prefer Vue 3 Composition API with TypeScript and explicit typed contracts.
- Keep the current React renderer boot path intact unless strictly required by the task.
- Keep imports/calls to `src/lib/api.ts` and `src/lib/browser.ts` compatible with current implementations.
- Make the smallest defensible diff that still leaves a usable migration substrate.

## Non-Goals

- Full React-to-Vue renderer migration for `apps/arrancador`.
- Rewriting route pages or React UI primitives to Vue in this task.
- Changing Tauri/Electron IPC command names or payload contracts.
- Removing React dependencies from `apps/arrancador`.

## Assumptions

- The user wants a migration-ready Vue substrate even if the app still boots through React today.
- Adding Vue- and Pinia-based source files under `apps/arrancador/src` is acceptable as long as they are not wired into the current React runtime.
- Root workspace Vue dependencies can be resolved by `apps/arrancador`; if local package dependencies are required later, that will be treated as a separate integration step unless needed for verification.

## Verification Plan

- Inspect current React tests and source to preserve behavior-critical semantics.
- Typecheck the affected app or targeted files if the local setup supports the new Vue imports.
- Run targeted verification for unchanged React tests only if the new files impact shared config or aliases.
- Produce `evidence.md`, `evidence.json`, and raw command outputs under the task folder.
- If verification fails, record the issue in `problems.md`, apply the smallest safe fix, and rerun verification.
