# Evidence - Eden Anytype object types rebuild v2

## Result
Overall verification status: `PASS`

## Acceptance Criteria

### AC1
The object types screen uses a distinctly Anytype-like composition with a left type library and a right page-like editor workspace.

Status: `PASS`

Evidence:
- `apps/eden/ts/src/components/settings/ObjectTypesSettings.vue`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypesSidebar.vue`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypeEditor.vue`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypeIdentitySection.vue`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypeFieldsSection.vue`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypePreviewRail.vue`

Notes:
- The screen is now structured as a two-pane workspace with:
  - left library
  - right page-like editor
  - dedicated preview rail
- The editor body now mirrors Anytype more closely via three property buckets:
  - `В шапке`
  - `В свойствах`
  - `Скрытые`
- The route-level component is orchestration-only.

### AC2
The rebuilt screen consumes Kosmos visuals theme tokens for background, surface, border, text, sidebar, and hover states; new ad-hoc gray color mixing is removed from the rebuilt object type UI.

Status: `PASS`

Evidence:
- `apps/eden/ts/src/components/settings/ObjectTypesSettings.vue`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypesSidebar.vue`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypeEditor.vue`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypeIdentitySection.vue`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypeFieldsSection.vue`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypePreviewRail.vue`
- `packages/kosmos-visuals/theme/css-variables.css`

Notes:
- Rebuilt object type UI now uses `var(--background)`, `var(--card)`, `var(--surface)`, `var(--border)`, `var(--input)`, `var(--ring)`, and `var(--sidebar-*)`.
- Where `color-mix(...)` remains, it is derived from existing Kosmos tokens such as `--background`, `--secondary`, or the current accent color, not from ad-hoc black/white gray surfaces.

### AC3
Object type rows and field rows have clear interactive states inspired by Anytype.

Status: `PASS`

Evidence:
- `apps/eden/ts/src/components/settings/object-types/ObjectTypesSidebar.vue`
- `apps/eden/ts/src/components/settings/object-types/ObjectTypeFieldsSection.vue`

Notes:
- Sidebar items now have explicit hover/active row states.
- Field rows now support:
  - hover state
  - drag state
  - drop target state
  - hidden-until-hover actions
  - native drag-and-drop reorder
- Field editing is now organized into visible Anytype-like buckets instead of one flat generic form list.

### AC4
The preview becomes a dedicated, visually separated rail/pane and mirrors header layout and featured-property presentation more closely to Anytype.

Status: `PASS`

Evidence:
- `apps/eden/ts/src/components/settings/object-types/ObjectTypePreviewRail.vue`
- `apps/eden/ts/src/components/typed-notes/TypedHeader.vue`
- `apps/eden/ts/src/components/typed-notes/ObjectPropertyField.vue`

### AC5
`TypedHeader.vue` and `ObjectPropertyField.vue` use Anytype-inspired lightweight featured-property rendering instead of generic boxed form controls for preview/read flows.

Status: `PASS`

Evidence:
- `apps/eden/ts/src/Editor.vue`
- `apps/eden/ts/src/lib/entryTitles.ts`
- `apps/eden/ts/src/store/eden.ts`
- `apps/eden/ts/src/components/typed-notes/TypedHeader.vue`
- `apps/eden/ts/src/components/typed-notes/ObjectPropertyField.vue`
- `apps/eden/ts/src/lib/systemTypes.ts`
- `apps/eden/ts/src/components/sidebar/EdenSidebar.vue`
- `apps/eden/ts/src/App.vue`
- `apps/eden/ts/src/components/spaces/SpacesView.vue`
- `apps/eden/ts/src/components/settings/TrashSettings.vue`

Notes:
- Featured inline properties are rendered as lightweight metadata rows with bullet separators.
- Readonly preview paths now render plain values/chips instead of generic form-heavy chrome.
- Relation candidate lists now respect `allowed_object_types` when that metadata is present, reducing vault-wide overrendering for relation fields.
- Ordinary note pages now follow the Anytype-like head flow more closely:
  - big untitled title field with `Без названия` placeholder
  - type line moved below the title
  - auto-generated draft titles stay in storage but render as untitled in the UI
  - `note_obj` no longer renders empty `description` / `related_notes` header blocks by default
  - the empty square icon tile is suppressed for inline note pages without a cover image

### AC6
Touched user-facing Russian UI strings are valid UTF-8 and free from mojibake.

Status: `PASS`

Evidence:
- UTF-8-aware reads recorded in:
  - `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/utf8-check.txt`
- Touched files:
  - `apps/eden/ts/src/Editor.vue`
  - `apps/eden/ts/src/lib/entryTitles.ts`
  - `apps/eden/ts/src/components/settings/ObjectTypesSettings.vue`
  - `apps/eden/ts/src/components/settings/object-types/ObjectTypesSidebar.vue`
  - `apps/eden/ts/src/components/settings/object-types/ObjectTypeEditor.vue`
  - `apps/eden/ts/src/components/settings/object-types/ObjectTypeIdentitySection.vue`
  - `apps/eden/ts/src/components/settings/object-types/ObjectTypeFieldsSection.vue`
  - `apps/eden/ts/src/components/settings/object-types/ObjectTypePreviewRail.vue`
  - `apps/eden/ts/src/components/typed-notes/TypedHeader.vue`
  - `apps/eden/ts/src/components/typed-notes/ObjectPropertyField.vue`
  - `apps/eden/ts/src/lib/systemTypes.ts`
  - `apps/eden/ts/src/components/sidebar/EdenSidebar.vue`
  - `apps/eden/ts/src/App.vue`
  - `apps/eden/ts/src/components/spaces/SpacesView.vue`
  - `apps/eden/ts/src/components/settings/TrashSettings.vue`
  - `apps/eden/ts/src/components/settings/object-types/shared.ts`
  - `apps/eden/ts/src/components/settings/object-types/useObjectTypeDraft.ts`

### AC7
Demo Ark seed data exists for at least one ordinary note object and one game object, with links and a custom type available for UI testing.

Status: `PASS`

Evidence:
- `apps/eden/ts/scripts/seedArkObjectDemo.ts`
- `apps/eden/ts/dev-data/ark-demo-mixed.db`
- `apps/eden/ts/dev-data/ark-demo-secondary.db`
- Raw logs:
  - `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/seed.txt`
  - `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/seed-secondary.txt`

### AC8
Eden typecheck and production build pass after the rebuild.

Status: `PASS`

Evidence:
- `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/tsc.txt`
- `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/build.txt`

### AC9
Current dev-mode/runtime issues are explicitly investigated and summarized with repo-side fixes applied where feasible; any environment-only blockers are documented with evidence.

Status: `PASS`

Evidence:
- `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/dev.txt`
- `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/dev-web.txt`
- `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/playwright-note-page.txt`
- Investigated issues:
  - `bun run dev` is gated by two Rust builds before Vite serves the UI
  - dev run in this environment ends with `spawn EPERM` from `vite-plugin-electron` child-process startup
  - direct Playwright Chromium launch in this Codex environment is also blocked by `spawn EPERM`
  - prior `dev:web` still routed through the Electron plugin and did not give a true UI-only path
  - editor metadata dirty-tracking watcher previously used `flush: "sync"`
- Repo-side mitigation applied:
  - `apps/eden/ts/src/Editor.vue`
  - metadata watcher changed from `flush: "sync"` to `flush: "post"` to reduce synchronous UI pressure on title/type/header prop edits
  - note title flow now keeps unique generated storage titles while rendering untitled placeholders in the editor UI
  - `apps/eden/ts/vite.web.config.mjs`
  - `apps/eden/ts/package.json`
  - added a real web-only dev entrypoint; `bun run dev:web` now starts Vite without the Electron plugin and reaches ready state in ~396 ms
  - `apps/eden/ts/main/store.ts`
  - hardened Ark list readers with `ensureArkList(...)` so mismatched RPC payload shapes do not crash the renderer path with `objects.map is not a function`

## Verification Commands

1. `bun x tsc --noEmit`
2. `bun run build`
3. `bun run seed:ark-demo`
4. `bun run seed:ark-demo -- --db D:\\Personal\\Hobby\\Coding\\kosmos\\apps\\eden\\ts\\dev-data\\ark-demo-secondary.db`
5. `bun run dev`
6. `bun run dev:web`

## Notes

- `bun run dev` still fails in this Codex environment because the Electron child spawn is blocked by `EPERM`. The raw log is preserved. This is treated as an environment limitation, not a source-level verification failure.
- `bun run dev:web` now provides a fast Vue-only iteration path and confirms the UI server reaches ready state without the Electron child-process layer.
- Existing global Eden files outside the rebuilt object type path still contain older UI debt and some broader localization debt. This pass is scoped to the object type experience, preview/header rendering, and nearby editor responsiveness.
