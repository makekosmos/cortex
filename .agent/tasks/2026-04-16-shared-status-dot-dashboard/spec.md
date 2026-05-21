# Task: Shared status dot for dashboard

## Goal

Create a reusable status dot component in `@kosmos/visuals` with click tooltip/popover behavior, darker success tone, and icon-like hover treatment, then use it in dashboard and keep it clear of native Windows controls.

## Acceptance Criteria

- AC1: `packages/kosmos-visuals` exports a reusable `StatusDot` component with tone variants, hover background, and click-open popover content.
- AC2: Dashboard uses the shared `StatusDot` instead of a local status button.
- AC3: The success/online green tone is darker than before and the dashboard dot sits safely left of Windows native controls.
- AC4: `apps/dashboard` still passes typecheck and direct Vite build verification.
