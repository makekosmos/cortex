# Eden editor quick reference

Scope: `products/eden/src/editor-cm/`, editor selection in `App.vue`.

- Editor path is CodeMirror (`CmEditor.vue`) for every current entry; there is no user-facing CM off switch and no TipTap fallback in `App.vue`.
- Canonical body storage is Markdown in ARK `content_json`: `{ "type": "markdown", "version": 1, "text": "..." }`. Use `editor-cm/content.ts` (`readEntryMarkdown` / `writeEntryMarkdown`) for editor and Markdown import/export paths.
- Keep `CmEditor.vue` lazy-loaded via `defineAsyncComponent(() => import('./editor-cm/CmEditor.vue'))`: main Eden bundle must stay small.
- CM title handling: normal notes use the title editor container; typed objects may render visible title/header via `TypedHeader`.
- Scroll-driven titlebar state must be based on the actually visible header/title container, not a hidden DOM node.
- Autosave/unmount saves must catch errors; no unhandled `void save()`/`await onSave` without error handling.
- Vim/CM behavior should not depend on object type unless explicitly required.

Related: `docs-site/apps/eden/typed-notes.md`, `docs-site/agents/forbidden/eden.md`.
