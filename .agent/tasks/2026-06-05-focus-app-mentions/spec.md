# 2026-06-05 — Focus app mentions

## Context

Focus Session is a built-in Shell command. The current "Блокировка" control is a dropdown over domain blocklists. The requested workflow is a textarea where the user can type `@` and choose an application, then Kosmos should prevent launching that application while focus is active.

App-level OS enforcement is documented as a future Digital Cave capability and is out of scope for this proof loop. This task implements the first focused capability: blocking app launches initiated through Kosmos app launcher / `app_index.launch` while an active Focus Session carries selected blocked app ids.

## Scope

In scope:

- Replace the Focus Session blocklist dropdown with a textarea-style app mention control.
- Populate mention suggestions from `app_index.list_all`.
- Persist selected app ids into Focus active state for the running focus session.
- Prevent `app_index.launch` from `LauncherView` when Focus is active and the app id is selected.
- Keep existing domain blocklist / hosts behavior untouched for existing settings and active-state flows where no app ids are supplied.

Out of scope:

- Killing already-running processes.
- Preventing apps launched outside Kosmos.
- New Windows service / elevated helper behavior.
- Schema migration or destructive ARK changes.

## Acceptance Criteria

**AC1.** Focus Session UI shows a textarea-like "Блокировка" field, not a dropdown, and selecting `@<app>` adds a visible app token/chip.

**AC2.** Starting focus with selected app mentions sends selected app ids through the focus session start path and stores them in Focus active state as additive JSON metadata (`blocked_app_ids`).

**AC3.** While Focus active state has `active=true` and includes an app id in `blocked_app_ids`, `LauncherView` refuses to call `app_index.launch` for that app and shows a Russian user-facing error.

**AC4.** Existing domain blocklist behavior remains compatible: `blocklist_id` still works, hosts blocking is still applied only through the existing helper/service path, and no direct hosts writes are added.

**AC5.** Verification evidence includes focused unit/regression coverage for the new DTO/blocking logic, typecheck/lint, focus/app visual verification, `ark:guard:writes`, and `ark:smoke` or a documented environment failure.
