# Problems

## AC2 — The primary top actions and footer/settings action remain reachable and visually stable with long recent/object-type lists; no content is clipped or made unreachable by the single-scroll layout.

- **Status:** UNKNOWN
- **Why it is not proven:** `products/eden/src/components/sidebar/EdenSidebar.vue` now uses one outer scroll container (`lines 17-77`) and keeps the footer in the same flow (`line 65`), but there is no current visual/manual proof with overflowing data that the top actions and footer remain reachable and stable.
- **Minimal reproduction steps:**
  1. Start Eden in the desktop shell/dev flow.
  2. Seed enough recent entries and note types to overflow the sidebar height.
  3. Open the notes or type-collection sidebar and scroll from top to bottom.
  4. Verify the top actions are reachable at the top and the footer settings action is reachable at the bottom without clipping.
- **Expected vs actual:**
  - Expected: a current visual verification artifact or direct local run proving reachability/stability under overflow.
  - Actual: only source inspection and build/test logs are available; no overflow verification artifact was provided.
- **Affected files:**
  - `products/eden/src/components/sidebar/EdenSidebar.vue`
  - `.agent/tasks/2026-06-14-eden-sidebar-scroll-icons/evidence.md`
- **Smallest safe fix:** Run and record the required visual overflow verification; attach a screenshot/video or explicit notes confirming the footer and top actions remain reachable.
- **Corrective hint:** This may not need a code change. First prove the current behavior with an overflowed sidebar dataset; if clipping appears, adjust the sidebar flex/spacing only in `EdenSidebar.vue`.

## AC9 — Visual behavior is verified with enough data to force overflow: multiple recent entries plus enough object types to exceed sidebar height. Verification must explicitly check that there is one sidebar content scrollbar and no inner scrollbars on `Недавние` / `Объекты`.

- **Status:** UNKNOWN
- **Why it is not proven:** The repository state shows the structural change for a single outer scroller, but there is no present-day visual verification artifact proving overflow behavior, one scrollbar, and absence of inner scrollbars on `Недавние` / `Объекты`.
- **Minimal reproduction steps:**
  1. Start Eden locally.
  2. Ensure the sidebar has many recent entries and many object types, including typed objects.
  3. Open DevTools or inspect the UI directly while scrolling the sidebar.
  4. Confirm the outer sidebar content scrolls, and `Недавние` / `Объекты` do not show their own vertical scrollbars.
  5. Confirm a recent typed object row has a neutral grey icon while its collection/type row is colored duotone.
- **Expected vs actual:**
  - Expected: explicit visual/manual verification for forced overflow and scrollbar behavior.
  - Actual: no screenshot, video, Playwright artifact, or verifier-run manual check exists for this criterion.
- **Affected files:**
  - `products/eden/src/components/sidebar/EdenSidebar.vue`
  - `.agent/tasks/2026-06-14-eden-sidebar-scroll-icons/evidence.md`
  - `.agent/tasks/2026-06-14-eden-sidebar-scroll-icons/evidence.json`
- **Smallest safe fix:** Perform the missing visual verification and save the artifact under this task directory; only change code if the live UI contradicts the intended one-scroll layout.
- **Corrective hint:** The code shape looks close, so verify before editing. If you add automated coverage, make it specifically assert one outer scroller and no nested overflow containers for the two groups.
