# 2026-05-28 — Eden scroll / drag-select stability

## Цель

Стабилизировать Eden editor UX вокруг обычного выделения текста, block-selection и scroll так, чтобы фикс не был очередной точечной заплаткой:

- browser-native text selection внутри `.ProseMirror` остаётся нативным и не перехватывается block-selection;
- block-selection остаётся доступным из gutter / пустой editor surface;
- scroll не замораживается и не прыгает после завершения block drag;
- автоскролл включается только для активного block drag у края scroll container.

## Контекст

Пользователь снова поймал резкий scroll вниз при выделении текста сверху заметки. Старый баг с `scroll-padding: 30vh` уже закрыт, новая причина — пересечение browser text-selection и кастомного `useBlockSelection`.

Важные внешние факты:

- MDN `scroll-padding`: свойство задаёт inset “optimal viewing region” scrollport и применяется ко всем scroll containers, а не только scroll snap.
- MDN `scrollIntoView`: browser alignment учитывает scroll margin / scroll region semantics.
- MDN Selection API: user selection — отдельный browser-level объект, меняется через `selectionchange` / `selectstart`.

## Scope

В scope:

- `extensions/eden/src/Editor.vue`
- `extensions/eden/src/Editor.css`
- `extensions/eden/src/App.css` только если нужен зеркальный CSS selector
- `extensions/eden/src/composables/useBlockSelection.ts`
- маленькие pure DOM helpers / tests для selection gate и class state
- postmortem / proof-loop evidence

Не в scope:

- переписывать TipTap schema / NodeViews;
- менять ARK writes / Eden persistence;
- менять визуальный стиль заметок;
- трогать unrelated shell/focus/settings изменения в worktree.

## Component Map

- `Editor.vue`: composition surface для editor DOM events, keyboard shortcuts, TipTap instance и class application.
- `useBlockSelection.ts`: state machine для rubber-band block selection и edge autoscroll.
- `blockSelectionPointer.ts`: pure DOM gate, решает кому принадлежит pointer gesture: native text selection или custom block-selection.
- `blockSelectionClasses.ts` (если понадобится): pure DOM/class helper, разделяет “drag active” и “selection mode”.

## Acceptance Criteria

**AC1. Native text selection ownership.** Mousedown/drag, начатый на text node или обычном block descendant внутри `.ProseMirror`, не вызывает `useBlockSelection.startTracking`, не вызывает `collapseEditorSelection`, не чистит `window.getSelection().removeAllRanges()` и не включает custom auto-scroll.

**AC2. Block selection entry points.** Block-selection стартует только из editor gutter / пустой editor surface и не стартует из интерактивных controls (`button`, `[role=button]`, `[role=checkbox]`, input-like NodeView controls).

**AC3. Scroll freeze lifecycle.** `.editor-wrapper` получает scroll-freeze только пока идёт active block drag. После `mouseup`, когда block selection остаётся выделенным для Delete/Ctrl+C, editor scroll снова доступен.

**AC4. Autoscroll bounds.** Custom edge autoscroll работает только при active block drag и только когда pointer находится в edge zone scroll container; после `finishDrag()` rAF loop остановлен.

**AC5. CSS invariant.** `.editor-wrapper` / mirrored `.editor-wrapper` styles не содержат large `scroll-padding-top/bottom`; breathing room остаётся через real layout (`padding-bottom`/spacer), а не scroll-padding.

**AC6. Regression coverage.** Есть automated regression coverage для AC1–AC4 на уровне pure helper/composable/browser component tests. Если часть поведения не покрывается jsdom/Vitest надёжно, это явно записано в evidence с manual/browser verification.

**AC7. Eden guard coverage.** Пройдены Eden-relevant проверки: `bun run --cwd extensions/eden test:unit`, `bun run --cwd extensions/eden test:vue`, `bun run --cwd shell build:extensions`, `bun run --cwd shell typecheck`, `bun run ark:guard:writes`, `bun run ark:smoke`, `bun run docs:check`, `bun run docs:sync`, финальный `bun run docs:check`.

**AC8. Postmortem/evidence.** `docs-site/agents/postmortems.md` описывает symptoms/root cause/fix/regression/prevention, а `.agent/tasks/2026-05-28-eden-scroll-drag-select/evidence.md` и `evidence.json` имеют PASS по всем AC.
