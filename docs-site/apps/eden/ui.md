# Eden UI quick reference

Scope: `products/eden/src/App.vue`, `App.css`, sidebar/search/settings/components visual changes.

- Root view: `products/eden/src/App.vue` composes `DesktopChrome`, top navigation, active screen, editor and titlebar slots.
- Home is «Всё»: the existing `notes` screen with `currentEntry === null`. Eden always starts there; only in-session Back/Forward history restores an editor or collection.
- `EverythingView` reads the already loaded `eden.entries`, shows only visible `note_obj` and `book_obj` entries, and sorts them by `updated_at DESC`; it must not introduce a route or a separate data API.
- The mixed feed uses CSS multi-column layout. `column-gap` and each card's bottom margin share one spacing token, cards avoid column breaks, covers keep their natural ratio, and narrow widths collapse to one column.
- Titlebar must use `<DesktopChrome>` slots; do not reintroduce custom safe-area/titlebar hacks.
- User-facing text is Russian; design tokens from `@kosmos/visuals` / CSS vars only.
- Titlebar page title is driven by editor scroll state (`titleOutOfViewChange`) and special object rules (e.g. images/person pages).
- Eden has no app sidebar. The titlebar navigation exposes only «Всё» and «Дневник» through the existing `openEverything()` / `openDiary()` actions; local Back/Forward uses `TitlebarHistoryControls`, not `vue-router`.
- For UI edits: run a targeted build/check and visually verify when possible; if not, report it explicitly.

Related: `docs-site/agents/forbidden/ui.md`, `docs-site/agents/forbidden/eden.md`.
