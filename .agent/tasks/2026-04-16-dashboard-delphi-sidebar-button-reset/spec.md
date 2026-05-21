# Task: Match dashboard sidebar button reset to Delphi

## Goal

Bring dashboard sidebar button appearance in line with `apps/delphi/ts` by applying the same global button reset semantics that Delphi relies on.

## Acceptance Criteria

- AC1: `apps/dashboard` global button reset matches Delphi closely enough that sidebar items rendered as `<button>` no longer inherit a native browser background.
- AC2: Dashboard renderer still passes typecheck and build after the reset fix.
