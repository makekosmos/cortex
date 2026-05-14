# Evidence - Eden settings and sidebar polish

## Summary
The Eden visual polish pass was completed in three areas:
- note/object title size now matches the settings page title scale
- shared sidebar buttons and project rows in `kosmos-visuals` are left-aligned and truncate with ellipsis
- Connected Apps settings were rebuilt onto the same section/row/action structure as the rest of Eden settings

Settings outer spacing was also moved into the shared settings content container, so individual settings pages no longer carry their own heavy outer padding.

## Code Evidence
- `apps/eden/ts/src/Editor.css`
  - `.title-input` now uses `28px`, `-0.4px`, `1.02` to match settings title rhythm
- `apps/eden/ts/src/App.css`
  - duplicated `.title-input` definitions were aligned with the same title scale so global overrides do not reintroduce the old larger heading
- `packages/kosmos-visuals/components/SidebarButton.vue`
  - button content now uses left alignment and explicit ellipsis behavior
- `packages/kosmos-visuals/components/Sidebar.vue`
  - project/object rows now justify content to the left and let labels shrink/truncate cleanly
- `apps/eden/ts/src/components/settings/ConnectedAppsSettings.vue`
  - rebuilt from bespoke card layout into `settings-tab` / `settings-section` / `settings-row` structure
- `apps/eden/ts/src/components/settings/SettingsPage.css`
  - shared settings content now owns outer padding
  - per-page `.settings-tab` outer padding was reduced to zero
  - action rows now wrap safely on smaller widths

## Verification

### AC1
PASS

Editor title input now uses the same effective title scale as settings page titles.

Evidence:
- `apps/eden/ts/src/Editor.css`
- `apps/eden/ts/src/App.css`

### AC2
PASS

Shared sidebar buttons now left-align content and truncate labels with ellipsis.

Evidence:
- `packages/kosmos-visuals/components/SidebarButton.vue`

### AC3
PASS

Shared sidebar project/object rows preserve left alignment and truncation.

Evidence:
- `packages/kosmos-visuals/components/Sidebar.vue`

### AC4
PASS

Connected Apps now follows the same settings page composition pattern as the rest of the settings area.

Evidence:
- `apps/eden/ts/src/components/settings/ConnectedAppsSettings.vue`

### AC5
PASS

Outer spacing for settings screens is now owned by `.settings-content`, while `.settings-tab` itself no longer adds large extra outer padding.

Evidence:
- `apps/eden/ts/src/components/settings/SettingsPage.css`

### AC6
PASS

Touched Russian UI strings were checked from UTF-8 reads and are free from mojibake.

Raw artifact:
- `raw/utf8-check.txt`

### AC7
PASS

Checks completed successfully:
- `bun x tsc --noEmit`
- `bun run build`

Raw artifacts:
- `raw/tsc.txt`
- `raw/build.txt`

## Notes
- `bun run build` still prints pre-existing Rust/Vite warnings, but the command completes successfully.
