# Evidence

- AC1: PASS
  - `apps/eden/ts/src/InlineCaret.ts` no longer uses the overlay/screen-position caret implementation.
  - The file is back to the prior widget-based structure with `Decoration.widget(...)`.

- AC2: PASS
  - `apps/eden/ts/src/Editor.css` is back to the prior native-caret fallback state:
    - `.ProseMirror.pm-inline-caret-enabled { caret-color: auto; }`
    - `.pm-inline-caret-anchor` remains visually hidden.

- AC3: PASS
  - The inline-caret implementation remains present in `apps/eden/ts/src/InlineCaret.ts`.

- AC4: PASS
  - `node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\typescript@5.8.3\node_modules\typescript\lib\tsc.js --noEmit -p D:\Personal\Hobby\Coding\kosmos\apps\eden\ts\tsconfig.json`

