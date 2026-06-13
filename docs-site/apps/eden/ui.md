# Eden UI quick reference

Scope: `products/eden/src/App.vue`, `App.css`, sidebar/search/settings/components visual changes.

- Root view: `products/eden/src/App.vue` composes `DesktopChrome`, sidebar, active screen, editor, titlebar slots.
- Titlebar must use `<DesktopChrome>` slots; do not reintroduce custom safe-area/titlebar hacks.
- User-facing text is Russian; design tokens from `@kosmos/visuals` / CSS vars only.
- Titlebar page title is driven by editor scroll state (`titleOutOfViewChange`) and special object rules (e.g. images/person pages).
- Sidebar lives in `components/sidebar/EdenSidebar.vue`; local Eden history controls use `TitlebarHistoryControls`, not `vue-router`.
- For UI edits: run a targeted build/check and visually verify when possible; if not, report it explicitly.

Related: `docs-site/agents/forbidden/ui.md`, `docs-site/agents/forbidden/eden.md`.
