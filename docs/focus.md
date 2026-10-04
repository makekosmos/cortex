# Focus

Focus is a **privileged Engine capability**: the ops and the OS-level
blocking live in cortex, while the session UI lives in the ordo app
(repo-split decision — `docs/repo-split-decisions.md` п.6). Apps interact
with focus only through Engine ops and manifest grants.

## Engine ops (`runtime/src/focus.rs`)

- `focus.list_blocklists` / `focus.upsert_blocklist` /
  `focus.delete_blocklist` — named domain/app lists, stored as ARK objects
  (soft-deleted via tombstones).
- `focus.get_active_state` / `focus.set_active_state` — current session
  state, JSON-encoded in `sync_kv` under `focus.active_state`
  (`FOCUS_ACTIVE_STATE_KEY`).
- `focus.resolve_blocklist_domains` — resolves `@list-id` references inside
  blocklists.
- `focus.ensure_presets` — seeds the preset blocklists on first use. Runs on
  every `focus.*` op, not just the first.

## Pomodoro (`runtime/src/pomodoro*`, `pomodoro_host.rs`)

`pomodoro.start|pause|resume|skip|stop|get_state` — timer ops exposed to
clients the same way.

## Rules

- State changes go through the write boundary: blocklists via ARK object
  writes (version-vector bumped), active state via `sync_kv`.
- OS blocking (process/window manipulation) stays inside the Engine; do not
  move privileged blocking into apps.
- New ops must be additive and grant-gated like the existing `focus.*` set.
