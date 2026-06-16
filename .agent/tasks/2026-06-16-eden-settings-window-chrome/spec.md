# Task Spec — Eden settings as a dedicated Kepler-settings-style window

## Task ID / Path

- `2026-06-16-eden-settings-window-chrome`
- `.agent/tasks/2026-06-16-eden-settings-window-chrome/spec.md`

## Original task statement

> Шестерёнка настроек в Eden теперь открывается (после фикса preload), но окно
> настроек встаёт точно поверх основного окна Eden и выглядит «внутри» него.
> Пользователь хочет: настройки открываются ОТДЕЛЬНЫМ окном с таким же визуалом,
> как у настроек Kepler (компактное центрированное окно, acrylic, не
> разворачивается на весь экран).

## Context / root cause

- Eden settings open via `kepler:eden-settings:open` → `openExtension("eden", "#/settings", "eden:settings")`.
- `openExtensionImpl` persists/restores window geometry in `extensions-data/<id>/window-state.json`, keyed by **`id`** (`"eden"`), not by `windowKey`. So the settings window (`windowKey="eden:settings"`) inherits the main Eden window's saved bounds and lands pixel-on-pixel over it, and dragging/resizing settings clobbers the main window's saved geometry.
- The Kepler shell settings window (`settings-window.ts`) is the visual target: fixed 880×560, centered on the primary display, `maximizable:false`, `fullscreenable:false`, acrylic backdrop, no geometry persistence.

## Scope

In scope:

- Give the Eden settings window a dedicated "settings" window profile when opened through the existing `kepler:eden-settings:open` path: centered, fixed compact size, non-maximizable, acrylic, and NOT persisting/clobbering the per-id `window-state.json`.
- Keep the existing command id / IPC namespace / `eden:settings` window key and the existing close path intact.
- Align the Eden settings renderer root background so the acrylic backdrop reads like the Kepler settings window.

Out of scope:

- Changing the default (non-settings) extension window behavior for any other window.
- New settings sections, routing redesign, or ARK/data/sync/schema/write-boundary changes.
- macOS-specific chrome tuning beyond what the shared helpers already provide.

## Assumptions

- "Same visual as Kepler settings" = same window silhouette/chrome: compact centered fixed-size window, acrylic translucency, non-maximizable — not a byte-for-byte renderer match.
- The Eden settings view already uses the shared `@kosmos/visuals` `SettingsSidebar`, so inner layout parity already exists; only the window chrome + root translucency need aligning.

## Constraints and non-goals

- Do not change `openExtension` behavior for `profile = "default"` callers.
- Do not alter command ids, IPC channel names, or the `eden:settings` window key.
- No regressions to headless/test mode (windows stay hidden, single-window invariant holds).
- Keep one logical change; no drive-by refactors of unrelated extension-host code.

## Acceptance Criteria

**AC1.** Opening Eden settings creates a separate window that does NOT reuse or overwrite the main Eden window's `extensions-data/eden/window-state.json`; the main Eden window keeps its position/size after opening and closing settings.

**AC2.** The Eden settings window opens centered on the primary display at the dedicated compact size (matching the Kepler settings window dimensions), with `maximizable:false` and `fullscreenable:false`.

**AC3.** The Eden settings window uses the acrylic backdrop material, and the settings renderer root is translucent enough that the backdrop reads like the Kepler settings window.

**AC4.** The existing open/close/reuse behavior still works: re-invoking `kepler:eden-settings:open` focuses the existing settings window (no duplicate), and the in-view close (`window.kepler.edenSettings.close()` / Esc) still closes it.

**AC5.** `profile = "default"` extension windows (Eden main, Delphi, Arrancador, etc.) are unchanged: they still restore/persist geometry and remain maximizable.

## Verification plan

- Unit: `bun test platform/desktop/electron/extension-window-profile.test.ts` — pure profile/geometry helpers (centering, settings traits: no-persist / non-maximizable / acrylic; default traits unchanged).
- Typecheck: `bun run --cwd platform/desktop typecheck`.
- Headed smoke (user): open Eden, click the gear → settings opens as a centered compact acrylic window; main Eden window bounds preserved after closing settings; re-open focuses, Esc closes.
