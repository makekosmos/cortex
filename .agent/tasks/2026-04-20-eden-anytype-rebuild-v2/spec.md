# Task Spec - Eden Anytype object types rebuild v2

## Goal
Rebuild Eden's object type/settings experience so it stops looking like a generic gray form UI and instead feels materially closer to Anytype in:
- spatial composition
- hierarchy
- row/item states
- preview behavior
- property editing feel
- motion/interaction cues

At the same time:
- consume Kosmos visuals theme tokens instead of inventing ad-hoc gray surfaces
- remove mojibake in touched UI
- produce test Ark databases for typed note and game flows
- document current dev-mode/runtime constraints and fix repo-side issues where feasible

## Sources to mirror
- `sample/anytype-ts-develop/src/ts/component/sidebar/page/type.tsx`
- `sample/anytype-ts-develop/src/ts/component/sidebar/preview.tsx`
- `sample/anytype-ts-develop/src/ts/component/page/elements/head/simple.tsx`
- `sample/anytype-ts-develop/src/ts/component/block/featured.tsx`
- `sample/anytype-ts-develop/src/scss/component/sidebar/page/type.scss`
- `sample/anytype-ts-develop/src/scss/component/sidebar/preview.scss`
- `sample/anytype-ts-develop/src/scss/component/headSimple.scss`
- `sample/anytype-ts-develop/src/scss/block/featured.scss`
- `packages/kosmos-visuals/theme/css-variables.css`
- `packages/kosmos-visuals/tokens/colors.ts`

## Design constraints
- Do not fake Anytype with random gradients and generic cards.
- Prefer Anytype-like section rhythm, hover overlays, list row shells, and lightweight borders.
- Reuse Kosmos visuals CSS variables for base colors and surfaces.
- Avoid introducing new mojibake. Validate touched strings in UTF-8-aware reads.
- Preserve Vue Composition API and keep route-level files as composition surfaces.

## Component map
- `ObjectTypesSettings.vue`
  route-level composition surface for the object type screen shell only
- `ObjectTypesSidebar.vue`
  left library with sections, search, row states, create action
- `ObjectTypeEditor.vue`
  right workspace shell with dedicated head/body sections
- `ObjectTypeFieldsSection.vue`
  field list with reordering affordances and row state handling
- `ObjectTypeIdentitySection.vue`
  name/icon/color/layout controls
- `ObjectTypePreviewRail.vue`
  sticky preview area driven by derived presentation data
- `ObjectPropertyField.vue`
  Anytype-inspired property cell rendering for featured/secondary layouts
- `TypedHeader.vue`
  Anytype-inspired object head for note/game pages
- `useObjectTypeDraft.ts`
  draft state and preview derivations

## Acceptance Criteria
- AC1: The object types screen uses a distinctly Anytype-like composition with a left type library and a right page-like editor workspace, not a generic form card layout.
- AC2: The rebuilt screen consumes Kosmos visuals theme tokens for background, surface, border, text, sidebar, and hover states; new ad-hoc gray color mixing is removed from the rebuilt object type UI.
- AC3: Object type rows and field rows have clear interactive states inspired by Anytype: hover overlay, active state, affordance for reordering, and tighter section rhythm.
- AC4: The preview becomes a dedicated, visually separated rail/pane and mirrors header layout and featured-property presentation more closely to Anytype.
- AC5: `TypedHeader.vue` and `ObjectPropertyField.vue` use Anytype-inspired lightweight featured-property rendering instead of generic boxed form controls for preview/read flows.
- AC6: Touched user-facing Russian UI strings are valid UTF-8 and free from mojibake.
- AC7: Demo Ark seed data exists for at least one ordinary note object and one game object, with links and a custom type available for UI testing.
- AC8: Eden typecheck and production build pass after the rebuild.
- AC9: Current dev-mode/runtime issues are explicitly investigated and summarized with repo-side fixes applied where feasible; any environment-only blockers are documented with evidence.

## Verification plan
- Typecheck:
  - `node node_modules/typescript/bin/tsc -p tsconfig.json --noEmit`
- Build:
  - `bun run build`
- Seed demo DBs:
  - `bun run seed:ark-demo`
  - `bun run seed:ark-demo -- --db apps/eden/ts/dev-data/ark-demo-secondary.db`
- Runtime/dev diagnostics:
  - run `bun run dev` and capture output
  - record repo-side errors versus environment-only spawn limits
- UTF-8 verification:
  - inspect touched files through UTF-8-aware reads, not PowerShell console rendering alone

## Raw artifact targets
- `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/tsc.txt`
- `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/build.txt`
- `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/seed.txt`
- `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/seed-secondary.txt`
- `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/dev.txt`
- `.agent/tasks/2026-04-20-eden-anytype-rebuild-v2/raw/utf8-check.txt`
