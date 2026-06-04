# Evidence — Delphi migration to @kosmos/visuals

Verified at: 2026-06-04

## AC1 — visuals shell/layout/common controls

Verdict: PASS

Evidence:

- `extensions/delphi/src/App.vue` uses `DesktopChrome appearance="settings"`, `IconButton`, and `TitlebarHistoryControls` from `@kosmos/visuals`.
- The Delphi titlebar now reuses the settings-style `DesktopChrome` layout: transparent drag area, no bottom border, and no ARK status dot.
- `extensions/delphi/src/components/SideBar.vue` uses `SettingsSidebar` / `SettingsSidebarButton` for all Delphi routes, including task views, so Delphi visually follows the same settings-style sidebar language.
- The Delphi manifest opts into the same native Electron backdrop used by Settings via `windowEffect: "acrylic"`.
- The settings-like transparency is visible on the sidebar: the renderer root and titlebar/header are transparent. Delphi opts into a stronger `SettingsSidebar` tone derived from visuals tokens (`color-mix(in srgb, var(--main-background-color) 64%, var(--color-bg-primary) 36%)`), and the right column owns the content background so the transparent titlebar visually matches the content surface.
- Delphi titlebar no longer renders the sidebar toggle or settings button; only history controls remain.
- Quick task creation follows a flatter Linear-like dark surface treatment: the QuickEntry panel and project dropdown no longer use foreground/white glow shadows, and Delphi floating "Новая задача" buttons no longer use `shadow-lg`.
- Sidebar buttons are non-selectable (`user-select: none`) through the shared `SettingsSidebarButton` and Delphi's renderer-level selection defaults.
- Delphi sidebar buttons use Phosphor icons in the same `duotone` / active `fill` pattern as the data table. The Delphi call sites pass `icon-variant="plain"` so the icon tile has no gradient background or inset shadow.
- Settings pages use `SettingsList`, `SettingsRow`, `SettingsButtonRow`, `Button`, `TextInput`, `Modal`, and `EmptyState` where matching visuals components exist.
- Delphi imports the visuals theme via `@kosmos/visuals/theme/css` in `extensions/delphi/src/global.css`.

## AC2 — no hardcoded local colors/fonts in Delphi renderer UI

Verdict: PASS

Command:

```powershell
rg --pcre2 -n "#[0-9A-Fa-f]{3,8}(?![0-9A-Fa-fA-Za-z_-])|rgb\(|rgba\(|@font-face|font-family|\b(bg|text)-(red|orange|yellow|green|blue|purple|pink|rose|emerald)-" extensions/delphi/src
```

Result: no matches.

Notes:

- QR code generation keeps named QR colors `black` / `white`; those are data passed to the QR generator, not renderer styling.

## AC3 — workflows remain renderable

Verdict: PASS for covered render paths

Commands:

```powershell
bun run --cwd shell build:extension delphi
```

Result: PASS.

Visual paths opened with Playwright using a narrow mock `window.kepler`:

- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-main-shell-1180x760.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-settings-general-1180x760.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-settings-spaces-1180x760.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-today-dark-settings-sidebar-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-today-dark-transparent-titlebar-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-today-dark-desktopchrome-settings-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-today-dark-sidebar-only-translucent-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-today-dark-acrylic-sidebar-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-sidebar-phosphor-plain-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-sidebar-mica-surface-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-titlebar-no-actions-sidebar-stronger-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-quick-entry-no-white-shadow-1228x768.png`

The visual run covered the main shell, settings general tab, and settings spaces tab. Worker evidence also covered QuickEntry/project dialog screenshots with the same mock approach.

## AC4 — UI rules preserved

Verdict: PASS

Evidence:

- User-facing text added in this integration pass is Russian.
- Titlebar/content/sidebar are built from `@kosmos/visuals` primitives.
- No new direct `addEventListener` calls were added in this integration pass.
- No nested interactive controls were added in this integration pass.

## AC5 — static checks and visual verification

Verdict: PASS

Commands:

```powershell
bun run lint
bun run --cwd shell build:extension delphi
git diff --check
```

Results: PASS.

Screenshot evidence:

- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-main-shell-1180x760.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-settings-general-1180x760.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-settings-spaces-1180x760.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-today-dark-settings-sidebar-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-today-dark-transparent-titlebar-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-today-dark-desktopchrome-settings-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-today-dark-sidebar-only-translucent-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-today-dark-acrylic-sidebar-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-sidebar-phosphor-plain-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-sidebar-mica-surface-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-titlebar-no-actions-sidebar-stronger-1228x768.png`
- `.tmp/visual/2026-06-04-delphi-kosmos-visuals/delphi-quick-entry-no-white-shadow-1228x768.png`

Additional computed-style check for the latest pass:

- `htmlBackground`: `rgba(0, 0, 0, 0)`
- `documentBodyBackground`: `rgba(0, 0, 0, 0)`
- DesktopChrome root background: `rgba(0, 0, 0, 0)`
- DesktopChrome titlebar/header background: `rgba(0, 0, 0, 0)`
- DesktopChrome right column background matches the content surface; DesktopChrome body itself is transparent.
- DesktopChrome titlebar leading buttons: `["Назад", "Вперёд"]`
- inner `SettingsSidebar` background: `color(srgb 0.0635761 0.0635967 0.0635986 / 0.744)`
- first sidebar button `userSelect`: `none`
- first Delphi sidebar icon background: `none / rgba(0, 0, 0, 0)`
- first Delphi sidebar icon box shadow: transparent
- QuickEntry panel `box-shadow`: `none`

Follow-up audit after the user reported that the generic app sidebar still looked unchanged:

```powershell
rg --pcre2 -n "#[0-9A-Fa-f]{3,8}(?![0-9A-Fa-fA-Za-z_-])|rgb\(|rgba\(|@font-face|font-family|\b(bg|text)-(red|orange|yellow|green|blue|purple|pink|rose|emerald)-|Sidebar as KosmosSidebar|<KosmosSidebar|project-page__fab|!bg-" extensions/delphi/src
```

Result: no matches.

## AC6 — no unrelated/release changes

Verdict: PASS

Evidence:

- No version bump or release metadata was changed.
- Existing untracked `ARCHITECTURE-PLANS.md` remains untouched.
- Changes are scoped to Delphi UI files and proof-loop artifacts.
