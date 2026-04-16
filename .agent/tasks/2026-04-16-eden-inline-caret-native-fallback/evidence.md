# Evidence

- AC1: PASS
  - `apps/eden/ts/src/Editor.css` no longer forces transparent caret color for `.ProseMirror.pm-inline-caret-enabled`.
  - The editor therefore falls back to the native browser caret.

- AC2: PASS
  - The inline-caret anchor is no longer visibly rendered in the editor surface (`display: none`), removing the visible widget layer that interfered with normal drag-selection UX.

- AC3: PASS
  - `apps/eden/ts/src/InlineCaret.ts` remains present and unchanged in the codebase.

- AC4: PASS
  - `node D:\Personal\Hobby\Coding\kepler\node_modules\.bun\typescript@5.8.3\node_modules\typescript\lib\tsc.js --noEmit -p D:\Personal\Hobby\Coding\kepler\apps\eden\ts\tsconfig.json`

