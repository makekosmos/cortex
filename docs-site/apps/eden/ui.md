# Eden UI quick reference

Scope: `products/eden/src/App.vue`, `App.css`, sidebar/search/settings/components visual changes.

- Root view: `products/eden/src/App.vue` composes `DesktopChrome`, top navigation, active screen, editor and titlebar slots.
- Home is «Всё»: the existing `notes` screen with `currentEntry === null`. Eden always starts there; only in-session Back/Forward history restores an editor or collection.
- `EverythingView` reads the already loaded `eden.entries`, shows only visible `note_obj` and `book_obj` entries, and sorts them by `updated_at DESC`; it must not introduce a route or a separate data API.
- The mixed feed uses CSS multi-column layout. `column-gap` and each card's bottom margin share one spacing token, cards avoid column breaks, covers keep their natural ratio, and narrow widths collapse to one column.
- Everything uses responsive independent columns: `+` stays first, the newest object starts to its right, and later objects can continue below `+` without reserving an empty first column.
- Note cards show a five-line plain-text body preview with a bottom fade; type and date metadata are intentionally omitted. Their visual radius matches book covers.
- Right-clicking a note or book card opens the shared flat `@kosmos/visuals` context menu. Its destructive «Удалить» item includes the existing trash icon and emits the entry id upward; cards never write to ARK directly.
- Titlebar must use `<DesktopChrome>` slots; do not reintroduce custom safe-area/titlebar hacks.
- User-facing text is Russian; design tokens from `@kosmos/visuals` / CSS vars only.
- The titlebar center shows «Всё» / «Дневник» navigation only on those two main pages. Object pages reuse the existing page-title presentation there instead: the object display title is always centered, and a person keeps the existing round image/icon beside the name.
- Eden has no app sidebar. Local Back/Forward uses `TitlebarHistoryControls`, not `vue-router`.
- `Shift+G` toggles an ephemeral 8px grid and live FPS counter over Eden's workspace. FPS drops are kept in the bounded session log `window.__edenFpsDrops`; attribution is limited to browser-observable long tasks, hidden-page throttling, or `unknown`. Inputs and the editor keep the shortcut to themselves.
- Creating an object switches away from Everything before IPC. The editor remains unmounted until the primary empty save succeeds, then the saved object is added to the in-memory Everything list; this preserves the existing autosave ordering without rendering the new card on the home page first.
- For UI edits: run a targeted build/check and visually verify when possible; if not, report it explicitly.

Related: `docs-site/agents/forbidden/ui.md`, `docs-site/agents/forbidden/eden.md`.
