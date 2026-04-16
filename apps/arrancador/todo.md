# Arrancador — implementation TODO (no websocket, no external runtime server)

### What is already done
- [x] Core pages wired in router and sidebar:
  - `Notifications`
  - `Download sources`
  - `Catalogue`
  - `Achievements`
  - `Downloads`
- [x] Backend API + Electron bridge handlers for:
  - `catalogue` CRUD + sync
  - `download_sources` CRUD
  - `achievements` events + seed
- [x] Local `Downloads` queue implemented in UI with `localStorage` persistence
- [x] `Notifications` and `Achievements` UI pages connected to API

### Stage 1 — catalogue, done on current pass
- [x] Implement `/catalogue` page:
  - create/edit/remove items
  - payload validation as JSON
  - search and source filtering
  - library sync button (`sync_library_to_catalogue`)

### Stage 2 — downloads UX and feature parity
- [x] Add explicit filters for `running/queued/paused/completed/failed/canceled`.
- [x] Add retry cooldown + retry counter visibility.
- [x] Add `clear completed/failed` and `clear all` actions.
- [x] Add optional task name/source presets for faster testing.

### Stage 3 — achievements improvements
- [x] Add compact KPI block: unlocked / total / locked.
- [x] Add timeline of last unlocked achievements.
- [x] Add context preview when recording events manually.
- [x] Optional export/import of achievement progress (JSON).

### Stage 4 — diagnostics + polish
- [x] Add per-page loading/error indicators with retry.
- [x] Add small empty states and guidance copy for first run.
- [x] Normalize notification messages and toast tones.
- [x] Smoke check for no WS dependency:
  - only bridge-backed IPC + local storage, no websocket stream on pages.

### Playwright coverage (local-first without runtime server)
- [x] `catalogue` page: create/edit/delete/search/filter/sync payload validation and empty/error handling.
- [x] `achievements` page: load states, manual recorder, timeline, KPI, import/export and retry path.
- [x] `downloads` page: presets, filter, pause/resume, retry cooldown, clear actions and queue persistence.

### Definition of done for each stage
- [ ] Backend command contract exists in `electron/main`.
- [ ] Frontend uses the contract in `src/lib/api.ts`.
- [ ] UI is accessible via router + sidebar.
- [ ] No websocket usage in page logic.
- [ ] No mocked API calls remain for these pages.
