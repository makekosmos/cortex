# Eden data quick reference

Scope: `products/eden/src/lib/kepler-api-shim.ts`, `lib/edenApi.ts`, `store/eden.ts`, import/search/data flows.

- Eden talks to ARK through `window.api` shim / `edenApi.ts`; components and store must not call `window.kepler.ark.request` directly.
- `EverythingView` and `BubbleDiaryView` stay in a two-entry Vue `KeepAlive` cache after their first mount. Navigation switches immediately; the global Eden ARK subscription updates `eden.entries` for «Всё», while the diary subscription keeps its timeline fresh without restarting migrations or list reads.
- «Всё» keeps its summary-first load: visible note cards and notes within a half-viewport overscan enter a 100 ms debounced queue with at most four concurrent `loadEntry(id, { contentOnly: true })` reads. This lightweight path uses the existing `get_object` RPC without loading links or type metadata, and caches plain text by `id + updated_at`; books and notes beyond that window do not trigger body reads.
- Notes and typed objects are ARK objects; object metadata lives in `header_props_json`, `header_layout`, `type_id`.
- Deleting an Everything card uses the existing ARK `deleteEntry` soft-delete path. After the local write succeeds, the Pinia store removes the entry from the in-memory list; a failed write leaves the card in place.
- Eden body `content_json` is Markdown storage (`{ type: "markdown", version: 1, text }`). Legacy ProseMirror/invalid bodies are read best-effort by `editor-cm/content.ts` and should only be rewritten through the normal ARK save path after editing.
- Search is ARK FTS5 (`search_objects`); «Всё» shows title/author matches immediately, then merges debounced full-text matches for visible notes and books without loading every body into the renderer. Do not resurrect ripgrep/Tantivy/Heart sidecar.
- Bulk imports that create graph edges must be staged: create/update nodes first, then write links after targets exist.
- Book metadata import accepts either a public HTTPS link or an ISBN. A saved ISBN is prefilled when the dialog opens. Direct ISBN input uses the fixed-origin Open Library edition endpoint; link import uses the same lookup after the source page yields a checksum-valid ISBN. It needs no API key, source-page values win, and lookup failure leaves the locally extracted preview intact. Imported language codes/names are normalized to the book schema's Russian select values before preview and apply.
- Any data-layer change must respect ARK write boundary and sync versioning rules.

Related: `docs-site/concepts/write-boundary.md`, `docs-site/concepts/ark-objects.md`, `docs-site/agents/forbidden/ark.md`, `docs-site/agents/forbidden/eden.md`.
