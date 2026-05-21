# Task: Dashboard status dot and shared content surface

## Goal

Match dashboard connection/readability indication to the minimal Delphi-style dot and extract reusable desktop content-surface spacing/radius settings into `@kosmos/visuals` so Delphi and Eden can reuse the same interior shell later.

## Acceptance Criteria

- AC1: Dashboard replaces the text status pill with a minimal dot indicator styled like the Delphi connection dot.
- AC2: `packages/kosmos-visuals` exports a reusable content surface/container for desktop chrome interior spacing and corner radius.
- AC3: Dashboard adopts the shared content surface instead of owning its padding/radius shell locally.
- AC4: `apps/dashboard` still passes typecheck and direct Vite build verification after the refactor.
