# Eden editor quick reference

Scope: `products/eden/src/editor-tiptap/`, `products/eden/src/editor-content/`, editor selection in `App.vue`.

- Editor path is TipTap (`TiptapEditor.vue`) for every non-image current entry; Eden has no legacy editor fallback or user-facing editor switch.
- Canonical current body storage is TipTap in ARK `content_json`: `{ "type": "tiptap", "version": 1, "doc": ... }`.
- Legacy markdown envelopes are still readable for import/old data through `editor-content/content.ts`.
- Markdown import/export paths should use `readEntryMarkdown` / `writeEntryMarkdown` from `editor-content/content.ts`.
- TipTap title handling: normal notes use the title input; typed objects may render visible title/header via `TypedHeader`.
- Scroll-driven titlebar state must be based on the actually visible header/title container, not a hidden DOM node.
- Autosave/unmount saves must catch errors; no unhandled `void save()`/`await onSave` without error handling.

Related: `docs-site/apps/eden/typed-notes.md`, `docs-site/agents/forbidden/eden.md`.
