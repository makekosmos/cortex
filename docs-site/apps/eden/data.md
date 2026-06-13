# Eden data quick reference

Scope: `products/eden/src/lib/kepler-api-shim.ts`, `lib/edenApi.ts`, `store/eden.ts`, import/search/data flows.

- Eden talks to ARK through `window.api` shim / `edenApi.ts`; components and store must not call `window.kepler.ark.request` directly.
- Notes and typed objects are ARK objects; object metadata lives in `header_props_json`, `header_layout`, `type_id`.
- Search is ARK FTS5 (`search_objects`); do not resurrect ripgrep/Tantivy/Heart sidecar.
- Bulk imports that create graph edges must be staged: create/update nodes first, then write links after targets exist.
- Any data-layer change must respect ARK write boundary and sync versioning rules.

Related: `docs-site/concepts/write-boundary.md`, `docs-site/concepts/ark-objects.md`, `docs-site/agents/forbidden/ark.md`, `docs-site/agents/forbidden/eden.md`.
