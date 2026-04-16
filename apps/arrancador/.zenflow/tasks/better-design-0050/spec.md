# Better-design Technical Specification

## Difficulty
Medium

## Technical context
- Frontend: React + TypeScript + Vite, Tailwind CSS, Recharts, lucide-react.
- Backend: Tauri (Rust) with SQLite, playtime tracking in `src-tauri/src/tracker.rs`.
- Current data: only `total_playtime` and `last_played` exist; no per-day/month history.

## Implementation approach
1. **Sidebar stability + UX refresh**
   - Make desktop sidebar width fixed and non-shrinking (remove `lg:w-auto`, add `min-w`/`max-w` + `shrink-0`).
   - Add `scrollbar-gutter: stable` (or equivalent) to prevent width shifts when scrollbars appear.
   - Persist collapsed state in local storage so it does not reset between route changes/reloads.
   - Tighten navigation layout (consistent padding, active indicator, subtle hover) to match the Raycast-inspired style.

2. **Playtime aggregation for flexible date ranges**
   - Add a new SQLite table `playtime_daily` with `(game_id, date, seconds)` and a unique `(game_id, date)` index.
   - Update the tracker loop to increment `playtime_daily` alongside `total_playtime`.
   - Implement a new Tauri command returning:
     - Daily totals for a requested date range.
     - Per-game totals for the same range (supporting “X–Y played”).
   - Expose the command via `src/lib/api.ts` with new TypeScript types.

3. **Statistics page charts + layout**
   - Replace the single “top played” bar chart with:
     - A range chart (daily totals across a chosen period).
     - A per-game breakdown for that same period (horizontal bars).
   - Add a range selector with presets (7/30/90 days) and optional custom dates.
   - Improve chart readability (grid opacity, tick labels, tooltip styling, gradient fills).
   - Use existing `Card` UI components to align spacing and hierarchy.

## Source code structure changes
- `src/components/Sidebar.tsx`: fixed width, persistent collapse state, refined nav styles.
- `src/pages/Layout.tsx`: ensure sidebar wrapper doesn’t affect layout width.
- `src/index.css`: optional helper class for `scrollbar-gutter: stable`.
- `src/pages/Statistics.tsx`: new chart layout, range selector, stats API integration.
- `src/lib/api.ts`: add `statsApi` (or similar) wrapper for the new Tauri command.
- `src/types/index.ts`: add `MonthlyPlaytime`, `GamePlaytime`, `PlaytimeStats` types.
- `src-tauri/src/database.rs`: create `playtime_daily` table on init.
- `src-tauri/src/tracker.rs`: insert/update per-day playtime rows.
- `src-tauri/src/stats.rs` (new): SQL aggregation queries for monthly and per-game totals.
- `src-tauri/src/lib.rs`: register the stats command.

## Data model / API changes
- SQLite table `playtime_daily`:
  - `game_id TEXT NOT NULL`
  - `date TEXT NOT NULL` (YYYY-MM-DD)
  - `seconds INTEGER NOT NULL`
  - UNIQUE(`game_id`, `date`) with FK to `games(id)`.
- New Tauri command: `get_playtime_stats(start?: String, end?: String)` returning `{ dailyTotals, perGameTotals }`.
- New frontend types mirroring the command response.

## Verification approach
- Frontend: `pnpm test` (if configured).
- Backend: `cargo test` in `src-tauri`.
- Manual: navigate across routes to confirm sidebar stability, verify charts render and month selector updates per-game breakdown.
