# Eden Remove CodeMirror

## Goal

Eden is TipTap-only. CodeMirror editor code, Vim/CM settings, CM tests, and CM package dependencies are removed. Shared content conversion stays available under a neutral module.

## Scope

- Move reusable content serialization helpers out of `src/editor-cm/`.
- Update Eden imports/tests/docs to the neutral content module.
- Remove CodeMirror/Vim editor UI, settings, tests, and dependencies.
- Verify no Eden runtime/test/package references to CodeMirror/Vim remain, except user-facing historical markdown content naming where needed.

## Acceptance Criteria

**AC1.** `products/eden/src/editor-cm/` no longer exists and no source/test import points to it.

**AC2.** Eden settings no longer expose Vim/CodeMirror settings or a Vim tab.

**AC3.** `products/eden/package.json` has no `@codemirror/*` or `@replit/codemirror-vim` dependencies.

**AC4.** Shared content helpers are available from a neutral path and all existing content/import/export tests still pass.

**AC5.** Verification passes: grep audit for CodeMirror/Vim leftovers, `bun run --cwd products/eden test`, `bun run products:build`, and formatting checks.
