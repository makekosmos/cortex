# SQOBA Technical Specification

## Difficulty
- Hard

## Technical context
- Tauri backend (Rust 2021) + React/TypeScript frontend.
- SQLite via `rusqlite`, settings stored in `settings` table.
- Windows-only runtime expectations.
- Current backup engine depends on Ludusavi manifest download.

## Goals
- Replace Ludusavi dependency with SQOBA (speed/quality optimized backup architecture).
- Persist per-game save location in DB with manual override.
- Backups only after game exit or on manual request.
- Compression enabled by default, with "skip compression next backup" option.
- Compression level configurable from 1 to 100 with speed/quality guidance.
- Max backups default 5, user-adjustable up to 100.

## Implementation approach
1. **SQOBA save discovery**
   - Add `save_path` per game in DB. If set, skip discovery.
   - Build a SQOBA manifest from local data in `example/ludusavi` (if present) and cache a JSON index in app data for fast lookups.
   - Implement Windows heuristics as fallback (Documents\My Games, Saved Games, `%AppData%`, `%LocalAppData%`, Steam `userdata`).
   - After discovery, persist `save_path` to avoid repeated scanning.

2. **Backup format + compression**
   - New archive format with `__sqoba_manifest.json` (versioned) storing original path, archive path, size, and mtime (hash optional).
   - Two modes:
     - Folder mode (uncompressed) for fastest backups.
     - Compressed archive using `tar` + `zstd` (multi-threaded) or `zip` with deflate.
   - Map compression level 1-100 to encoder range; document speed trade-offs in UI.
   - Add settings keys: `backup_compression_enabled`, `backup_compression_level`, `backup_skip_compression_once`.

3. **Backup decisions + restore**
   - Update `check_backup_needed` to compare latest file mtimes or a stored fingerprint rather than compressed size.
   - Update `check_restore_needed` to use uncompressed size from manifest/DB.
   - Restore supports folder, compressed SQOBA archives, and existing Ludusavi backup mappings for compatibility.

4. **Settings + API + UI**
   - Remove backup-before-launch flow and setting.
   - Add save-path editor in `GameDetail` (input + folder picker + status).
   - Add compression controls in Settings: enable toggle, level slider/input, skip compression next backup action, and hint text.
   - Clamp max backups to 1..100 in UI and backend.

## Source code structure changes
- `src-tauri/src/backup/engine.rs`: refactor into SQOBA engine + compression support.
- New modules (names TBD): `src-tauri/src/backup/save_locator.rs`, `src-tauri/src/backup/sqoba_manifest.rs`, `src-tauri/src/backup/archive.rs`.
- `src-tauri/src/backup.rs`: integrate SQOBA settings, skip-once flag, progress stages.
- `src-tauri/src/database.rs`: add `save_path` column + migration; add compression defaults.
- `src-tauri/src/games.rs`: include `save_path` in structs/queries; allow updates.
- `src-tauri/src/settings.rs`: include compression fields and clamp `max_backups_per_game`.
- `src/lib/api.ts`, `src/types/index.ts`: add `save_path` and compression settings; allow per-backup override if needed.
- `src/pages/GameDetail.tsx`: save path UI + removal of pre-launch backup prompts.
- `src/pages/Settings.tsx`: compression UI + max backup cap.
- Tests in `src-tauri/src/backup/*` and/or `src-tauri/tests/*`.

## Data model / API changes
- `games.save_path TEXT` for save location (manual or cached).
- Settings keys: `backup_compression_enabled`, `backup_compression_level`, `backup_skip_compression_once`.
- Optional `backups.uncompressed_size` (or read from manifest) for restore decisions.
- `update_game` accepts `save_path`.
- `create_backup` optionally accepts per-run compression override, or respects skip-once flag.

## Verification
- `cargo test` in `src-tauri`.
- Manual smoke checks: create compressed/uncompressed backup, restore, manual save path override.

## Open questions
- The repo's `example` directory is gitignored and absent locally. Where should the SQOBA manifest source live?
- Should `save_path` support multiple locations (array/JSON) or a single root only?
- Preferred archive format: `.sqoba` (tar+zstd) or `.zip`?
- Do you want Windows registry backup support?
