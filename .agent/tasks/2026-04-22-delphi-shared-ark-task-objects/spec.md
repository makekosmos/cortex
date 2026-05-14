# Delphi Shared Ark Task Objects

## Original Task Statement

User request summary:

- explain how data currently gets into Ark because it is a black box right now;
- make Delphi use the same currently selected Ark database/space as the rest of Kosmos, especially Eden;
- ensure Delphi tasks are written into that shared Ark database so they are visible and openable from Eden;
- preserve the domain model direction that tasks are Ark objects with fields, with nested tasks/projects intentionally deferred for now;
- verify the scenario with Playwright in headless/background mode so app windows do not disturb the user.

## Summary

Bring Delphi Electron onto the shared Ark selection contract already used by the rest of Kosmos, and persist Delphi tasks into Ark's object model instead of leaving them only in Delphi-local tables.

The implementation should make a Delphi-created task land in the currently selected shared `ark.db`, under an Ark object type that Eden can list and open from the same selected space. Delphi must still keep its own task UI working during the transition.

## Acceptance Criteria

- AC1: Delphi Electron resolves its active space from the shared selected-space contract in `appData/Kosmos/selected-space.json`, so when Eden has selected a personal/shared space, Delphi boots against that same Ark DB without requiring a separate local choice.
- AC2: Delphi space changes continue to update the shared selected-space marker, and fallback behavior remains intact when no shared selected space exists.
- AC3: Saving or updating a Delphi task writes an Ark object into the currently selected shared Ark DB, not only Delphi-local task rows.
- AC4: The Ark object written by Delphi uses a dedicated object type definition for Delphi tasks and stores the current Delphi task fields needed for round-trip visibility in Eden:
  - title
  - description/notes
  - completion/cancel/trash state
  - scheduling/deadline/reminder fields
  - priority
  - ordering / creation metadata
  - project/heading/area/tag references as plain metadata fields where applicable
- AC5: Eden, when pointed at the same selected space, can list and open a Delphi-created task via Ark object APIs from the shared `ark.db`.
- AC6: Delphi remains operational for its existing task UI after the Ark object write path is added; the change must not break current Pinia/local DB hydration flows.
- AC7: The repository contains an automated Playwright verification for the shared-space Delphi -> Ark -> Eden flow, and the verification runs in background/headless-friendly mode so the user does not get foreground application windows during the test run.
- AC8: The implementation leaves nested tasks / task-as-project semantics explicitly deferred; there must be no partial or misleading nested-task data model pretending that hierarchy already works.

## Constraints

- Keep the diff focused on Delphi/Eden/Ark integration needed for this flow.
- Prefer additive changes over broad refactors.
- Do not remove Delphi's current local task model in this task unless strictly necessary for compatibility.
- Use the existing shared selected-space helper in `packages/shared-space/selectedSpace.ts` rather than inventing a second cross-app contract.
- Use Ark's existing object model and RPC surface (`upsert_object`, `list_objects`, `get_object_type`, `upsert_object_type`) rather than bypassing Ark with direct SQLite writes from UI code.
- Preserve current fallback behavior for users who have no shared selected space yet.

## Non-Goals

- Full task hierarchy / subtasks / project nesting semantics.
- Replacing Delphi's whole internal task store with Eden's entry model.
- Building a polished Eden-specific Delphi task editor UX in this task.
- Cross-platform Delphi Kotlin/Swift parity.
- Relay/P2P sync redesign.

## Assumptions

- Eden can already surface arbitrary Ark object types via `list_objects` / `get_object`, so a Delphi-specific task object type is sufficient for first visibility.
- For this task, "visible in Eden" means the object appears in Eden entry lists and can be opened from the same shared Ark DB; perfect custom field rendering in Eden is not required yet.
- Projects and nested tasks are intentionally frozen as future work; current persistence may only preserve flat metadata references.

## Verification Plan

1. Run focused Delphi TS tests that cover existing task/store behavior.
2. Run focused Ark Rust tests if Ark schema/object helpers are changed.
3. Add and run a Playwright flow that:
   - creates an isolated temp appdata/home environment;
   - writes a shared selected-space marker;
   - launches Delphi in background/headless-friendly mode;
   - creates a task through the app or its exposed APIs;
   - verifies the task exists in the selected shared `ark.db` as an Ark object;
   - launches Eden against the same selected space;
   - verifies Eden can list/open that task;
   - cleans up the temp environment.
4. Record results in `.agent/tasks/2026-04-22-delphi-shared-ark-task-objects/evidence.md`, `evidence.json`, and raw artifacts.
