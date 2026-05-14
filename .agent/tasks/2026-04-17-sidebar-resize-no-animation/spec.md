# Task Spec: Remove animation during manual sidebar resize

## Original Task

Давай уберем анимацию при ресайзе ручном, а то сейчас эта задержка раздраажет. Не на скрытии / раскрытии сайдабара

## Scope

Remove the width animation only while the user is manually dragging the shared sidebar resize handle.

## Assumptions

- The resize lag comes from the width transition on `.kosmos-sidebar-wrapper`.
- Hide/show animation should remain unchanged.

## Constraints

- Make the smallest safe change in shared sidebar styling.
- Do not change the sidebar hide/show animation behavior.

## Non-goals

- Redesigning sidebar motion timing globally.
- Changing resize logic in JavaScript.

## Acceptance Criteria

- AC1: Manual resize of the shared sidebar does not animate width while dragging.
- AC2: Hide/show animation of the shared sidebar remains intact.
- AC3: Focused consumer type checks still pass after the styling change.

## Verification Plan

1. Update shared sidebar CSS so `.is-resizing` disables width transition.
2. Confirm hide/show animation rules remain untouched.
3. Run focused type checks for current sidebar consumers.
