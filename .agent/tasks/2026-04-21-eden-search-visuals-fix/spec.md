# Eden Search And Visuals Fix

## Summary

Investigate why Eden search returns no results and why the search surface looks detached from the shared Kosmos Visuals language, then apply the smallest safe fix that restores search for current Eden data and moves the search UI onto the shared visuals contract.

## Original Task

Изучи устройство EDNTS, изучи, как он именно оформлен, и изучи Kosmos Visuals. Сейчас по какой-то причине Эден, видимо, не использует поисковик и Kosmos Visuals, потому что выглядит он странно, не так, как должно было быть. То есть не использует компонент Kosmos Visuals. И также сам поисковик почему-то не использует Tivity, либо я не знаю, что с ним, потому что поиск по... То есть я пытаюсь найти что-то, что точно существует, и он этого не находит. Он вообще ничего не находит, абсолютно. Абсолютно ничего не находит.

## Findings Before Implementation

- Eden shell already uses shared `DesktopChrome`, `DesktopContentSurface`, and `TitlebarHistoryControls` from `@kosmos/visuals`.
- Eden search UI is still a local `src/components/SearchOverlay.vue` with custom CSS instead of a shared visuals search surface.
- Eden search data path currently calls only `eden-heart` (`main/store.ts -> searchEntries()`), while current note saves default to Ark object storage (`note_obj`), so searchable visible objects are excluded from search results.

## Acceptance Criteria

- AC1: Eden search returns matches for current user-visible Eden notes and objects, including Ark-backed `note_obj` entries created through the current save path.
- AC2: Eden search still returns matches for legacy `eden-heart` entries, and the combined search result format remains compatible with the existing renderer contract (`entryId`, `text`, `line`, `file`).
- AC3: Eden search UI is rendered through a shared `@kosmos/visuals` component rather than the old standalone custom overlay shell.
- AC4: Automated verification covers both the search behavior and the UI integration, including at least one Ark-backed search case.

## Constraints

- Keep search evolution on the backend side for Ark data instead of adding a new renderer-side ad hoc filter.
- Preserve the current Electron preload / IPC contract shape exposed to the renderer.
- Keep the fix scoped to search behavior and the search surface; do not refactor unrelated Eden shell areas.
- Prefer the smallest defensible diff that restores behavior and visual consistency.

## Non-Goals

- Do not redesign all of Eden around Kosmos Visuals.
- Do not replace `eden-heart` search with a new engine.
- Do not introduce a broad search ranking overhaul beyond what is needed for this regression.

## Assumptions

- The user's “Tivity” reference means the Rust/Tantivy-backed search expectation or, more broadly, the backend search layer rather than a renderer-only filter.
- Ark-backed objects are the dominant current save path for Eden notes and therefore must participate in search.

## Verification Plan

1. Run Eden lint/build after the code changes.
2. Run focused Eden e2e coverage for search.
3. Record command outputs and touched-file evidence in the task artifact folder.
