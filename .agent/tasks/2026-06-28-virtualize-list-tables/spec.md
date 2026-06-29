# Task: Virtualize the three uniform-height list tables

Status: **READY TO IMPLEMENT** (run locally — requires visual verification that the
remote/web session could not perform).
Branch: continue on `claude/vue-perf-optimization-hp1jhk` (or a child branch).
Classification: `FULL_LOOP` (UI behavior change, must be visually verified).

## Why this task exists

Perf pass on the Kosmos Vue UI. `v-memo` was already applied to the large lists
(commit `bdcda8c`). The remaining high-impact win is **list virtualization** — rendering
only the visible rows instead of the whole collection. This was NOT done in the remote
session on purpose: virtualization changes scroll/selection behavior and depends on a
fixed row height, and the remote environment cannot visually verify the result. Do it
locally where you can run the desktop shell and watch the scroll.

There is a proven, in-repo virtualization reference to mirror:
`products/eden/src/components/sidebar/EdenSidebar.vue` (recent list, template lines ~93–134).
Study it first.

## Scope — exactly three tables

All three have **single-line cells** (`white-space: nowrap` + `text-overflow: ellipsis`)
and a `min-height: 36px` row, so they are safe to convert to a fixed 36px row and
window with simple index math.

| # | Component | Scroll container | Row `v-for` | Row rule to fix |
|---|-----------|------------------|-------------|-----------------|
| 1 | `platform/desktop/src/dashboard/ObjectTable.vue` | `.object-table__body` | `row in rows` (~line 41) | `.object-row` `min-height: 36px` → `height: 36px` (~line 109) |
| 2 | `platform/desktop/src/dashboard/UsageTable.vue` | `.usage-table__body` | `row in rows` (~line 66) | `.usage-row` `min-height: 36px` → `height: 36px` (~line 165) |
| 3 | `products/eden/src/components/objects/TypeObjectsView.vue` | `.type-objects-table-body` | `entry in collectionEntries` (~line 161) | `.type-objects-row` `min-height: 36px` → `height: 36px` (~line 388) |

`ROW_HEIGHT = 36`. Each of these `v-for` rows already carries a `v-memo` (added in
`bdcda8c`) — **keep it**; v-memo and virtualization coexist fine.

## Explicitly OUT OF SCOPE (do NOT virtualize here — separate tasks)

- `products/eden/src/components/SearchOverlay.vue` — preview uses `line-clamp-2`, so rows
  are **variable height** (1–2 lines). Needs measured-height virtualization or a design
  change to a fixed row; not a simple height fix.
- `platform/desktop/src/command-host/CommandListView.vue` — accessories `flex-wrap: wrap`
  + subtitle → variable height, plus it has **sections** and **keyboard navigation /
  scroll-into-view**. Hardest case. Leave it (it already has `v-memo`).

## Reference pattern (from EdenSidebar)

EdenSidebar renders a spacer of full height and positions only the visible slice with
`transform: translateY(...)`. Its constants: `RECENT_ITEM_HEIGHT = 56`,
`RECENT_MONTH_HEADER_HEIGHT = 26`, `RECENT_ITEM_GAP = 4`, `RECENT_LIST_OVERSCAN = 6`;
`syncRecentViewport` is the `@scroll` handler; `findRecentRowIndex()` is a binary search
(needed only because EdenSidebar mixes row heights). Our three tables are **uniform
height**, so we do NOT need the binary search — plain integer math is enough.

## Implementation steps (per table, mirror across all three)

1. **Fix the row height**: change the row rule from `min-height: 36px` to `height: 36px`.
   Cells are already `nowrap` + ellipsis, so visually this should be a no-op — confirm.

2. **Add windowing state** in the component (or a small `useVirtualRows` composable if you
   want to share across the three):
   - `const ROW_HEIGHT = 36;`
   - `const OVERSCAN = 6;`
   - `const scrollTop = ref(0);`
   - `const viewportHeight = ref(0);`
   - scroll container `ref` (e.g. `bodyRef`).

3. **Computeds**:
   - `totalHeight = computed(() => rows.value.length * ROW_HEIGHT)`
   - `startIndex = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - OVERSCAN))`
   - `endIndex = computed(() => Math.min(rows.value.length, Math.ceil((scrollTop.value + viewportHeight.value) / ROW_HEIGHT) + OVERSCAN))`
   - `visibleRows = computed(() => rows.value.slice(startIndex.value, endIndex.value).map((row, i) => ({ row, top: (startIndex.value + i) * ROW_HEIGHT })))`

4. **Template**: wrap the rows in a spacer of `height: totalHeight px`; render only
   `visibleRows`, each with `:style="{ position: 'absolute', top: 0, transform: 'translateY(${top}px)', height: ROW_HEIGHT+'px' }"` (or position via the spacer). The scroll
   container needs `position: relative; overflow: auto;` and the spacer `position: relative`.
   Keep all existing per-row bindings, classes, `:key`, `v-memo`, `@click`,
   `@keydown`, `tabindex`, `data-testid`, and the `kosmos-scroll` class.

5. **Scroll + resize wiring**:
   - `@scroll="onScroll"` on the body; `onScroll` sets `scrollTop.value = bodyRef.value.scrollTop`.
   - Measure `viewportHeight` from `bodyRef.clientHeight` on mount and on resize
     (`ResizeObserver`).
   - **Clean up** the `ResizeObserver` (and any listeners) in `onBeforeUnmount` — this is a
     hard project rule (no `addEventListener`/observers without cleanup).

6. **Testability**: mirror EdenSidebar's debug attributes on the scroll container, e.g.
   `:data-total-count="rows.length"`, `:data-rendered-count="visibleRows.length"`,
   `:data-range-start="startIndex"`, `:data-range-end="endIndex"`, plus a
   `data-testid="...-virtual-list"`. Useful for an e2e assertion that rendered << total.

## Guardrails / never-break

- Sticky table **header stays outside** the virtualized body — virtualize only the body.
- Preserve keyboard handlers on `TypeObjectsView` rows (`@click`, `@keydown.enter/.space`,
  `tabindex`, `role`).
- No hardcoded colors/fonts — use `@kosmos/visuals` tokens (no new `#hex`/`rgb()`).
- No `addEventListener`/`ResizeObserver` without `onBeforeUnmount` cleanup.
- Keep the existing `v-memo` arrays unchanged.
- One logical change per commit; do the three tables together OR one commit each — your call.

## Verification

1. `bun run --cwd platform/desktop typecheck` — no NEW errors. (Baseline has exactly one
   pre-existing error: `error TS2688: Cannot find type definition file for 'vite/client'`.)
2. Eden tests: `bun test products/eden/tests` — baseline is **22 fail / 13 errors**
   (pre-existing); do not introduce new failures.
3. Desktop command tests still green: `bun test tests/unit/command-*.test.ts tests/unit/raycast-api.test.ts`.
4. **Visual (the reason this is a local task)** — launch the desktop shell and for each table:
   - Scroll top→bottom: smooth, no row jitter/jumping, no blank gaps at speed.
   - Rows align (no overlap, no double-height) — confirms the 36px fix holds.
   - Click / keyboard-activate a row → opens the **correct** row (index math correct).
   - Nothing clipped or misaligned vs. before.
   - Large dataset (hundreds–thousands of rows) renders fast; DOM node count stays small
     (inspect: rendered rows ≈ viewport + 2·overscan, not the whole list).

## Done when

All three tables virtualized, the four verification steps pass, visual checklist confirmed
locally, committed and pushed.
