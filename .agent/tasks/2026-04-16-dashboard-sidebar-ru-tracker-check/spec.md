# Task: Dashboard sidebar parity, Russian localization, tracker verification

## Goal

Bring `apps/dashboard` closer to the shared Kosmos desktop shell by reusing the Delphi/Eden sidebar pattern, keeping the sidebar fixed on the left, translating the dashboard UI to Russian, and verifying that the installed `usage-tracker` is actively writing to Ark DB on the current machine.

## Acceptance Criteria

- AC1: `apps/dashboard` uses the same `KosmosSidebar` configuration pattern as Delphi/Eden, including persisted sidebar config and a fixed left rail that does not scroll away with content.
- AC2: User-facing dashboard copy is translated to clean Russian across shell, overview, sessions, tables, charts, and formatting helpers.
- AC3: Dashboard styling continues to rely on shared `@kosmos/visuals` theme variables and keeps the content scroll separate from the sidebar.
- AC4: `apps/dashboard` passes renderer typecheck and build checks in the current repository environment.
- AC5: The currently installed `usage-tracker` process and live Ark DB state are verified from the local machine, with evidence captured from the actual runtime.
