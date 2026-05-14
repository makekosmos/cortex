# Task: Kosmos visuals titlebar chrome

## Goal
Add a unified desktop titlebar layer to `packages/kosmos-visuals` that composes cleanly with the shared sidebar: the titlebar sits above the sidebar, adapts to macOS vs Windows control placement, and becomes the home for top chrome actions such as sidebar toggle buttons.

## Acceptance Criteria
- AC1: `packages/kosmos-visuals` exports a reusable titlebar component with OS-adaptive layout rules: macOS reserves left traffic-light safe area for leading content, Windows keeps leading content flush-left and places window controls on the right.
- AC2: `packages/kosmos-visuals` exports a reusable desktop chrome/layout component that places the titlebar above the sidebar/content area so the sidebar stops at the bottom edge of the titlebar instead of flowing underneath it.
- AC3: `apps/dashboard` adopts the shared titlebar/chrome layout, moves sidebar-top actions into the titlebar, and keeps the sidebar fixed on the left while only the main content scrolls.
- AC4: `apps/dashboard` wiring supports titlebar window actions on Windows through the existing preload bridge and keeps the layout safe on macOS.
- AC5: `apps/dashboard` still passes typecheck and build verification after the shared chrome migration.
