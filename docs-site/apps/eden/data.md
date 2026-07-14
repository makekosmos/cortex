# Eden data quick reference

Scope: `products/eden/src/lib/kepler-api-shim.ts`, `lib/edenApi.ts`, `store/eden.ts`, import/search/data flows.

- Eden talks to ARK through `window.api` shim / `edenApi.ts`; components and store must not call `window.kepler.ark.request` directly.
- `EverythingView` and `BubbleDiaryView` stay in a two-entry Vue `KeepAlive` cache after their first mount. Navigation switches immediately; the global Eden ARK subscription updates `eden.entries` for «Всё», while the diary subscription keeps its timeline fresh without restarting migrations or list reads.
- Notes and typed objects are ARK objects; object metadata lives in `header_props_json`, `header_layout`, `type_id`.
- Eden body `content_json` is Markdown storage (`{ type: "markdown", version: 1, text }`). Legacy ProseMirror/invalid bodies are read best-effort by `editor-cm/content.ts` and should only be rewritten through the normal ARK save path after editing.
- Search is ARK FTS5 (`search_objects`); do not resurrect ripgrep/Tantivy/Heart sidecar.
- Bulk imports that create graph edges must be staged: create/update nodes first, then write links after targets exist.
- Any data-layer change must respect ARK write boundary and sync versioning rules.

Related: `docs-site/concepts/write-boundary.md`, `docs-site/concepts/ark-objects.md`, `docs-site/agents/forbidden/ark.md`, `docs-site/agents/forbidden/eden.md`.
