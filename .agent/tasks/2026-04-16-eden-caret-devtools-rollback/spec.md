# Task: Roll back Eden caret one more step and restore devtools access

## Goal
Return Eden to the earlier editor-caret behavior from before the native-caret fallback, while also restoring a reliable way to open devtools in the Electron shell.

## Component Map
- `apps/eden/ts/src/Editor.css`: restore the pre-fallback custom inline-caret styling.
- `apps/eden/ts/src/InlineCaret.ts`: keep the old widget-based inline caret and remove the fallback-only behavior; allow the widget to render without forcing the previous `contenteditable="false"` path.
- `apps/eden/ts/main/main.ts`: restore a devtools entry path equivalent to Delphi via Electron menu/shortcut wiring.

## Acceptance Criteria
- AC1: Eden no longer uses the native-caret fallback styling inside the ProseMirror editor.
- AC2: The custom inline caret is rendered again through the existing widget-based implementation.
- AC3: Eden devtools can be opened through the main-process chrome contract (`F12` / standard View menu path).
- AC4: TypeScript verification passes for `apps/eden/ts`.
