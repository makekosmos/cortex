# Arrancador Test Strategy & Contracts

## Test layers (what goes where)

- **Frontend unit**: Pure utilities and parsing in `src/lib` and `src/types` (no DOM).
- **Frontend component**: Page and component behavior in `src-vue/pages` and `src-vue/components` with Vue Test Utils; mock `window.arrancador` and local helpers.
- **Frontend integration**: Pinia stores and composables in `src-vue` that combine API + state (still mocked API/bridge).
- **Electron main unit**: Pure logic in `electron/main/services/*` and helpers with temp files/db fixtures.
- **Stats read-model unit**: Ark-backed playtime source wiring in `electron/main/services/playtime-stats.ts` with legacy-fallback fixtures.
- **Electron main integration**: SQLite-backed flows in `electron/main/db/**` and service modules using temp DB/files.
- **E2E smoke (optional)**: Electron bridge + navigation sanity (minimal flows only).

## Contract boundaries

- **Frontend <-> Backend**: `src/lib/api.ts` is the single invoke surface. Any change to a command name, payload shape, or return type is a breaking change and must be mirrored here.
- **Backend public API**: Electron main-process handlers are exposed through the bridge and should return `Result<T, String>` semantics at the contract layer. Playtime statistics stay behind a narrow repository adapter so the storage source can change without affecting the renderer API.
- **Error surface**: Electron main-process errors become rejected `invoke` promises; contract tests should assert on error category and message prefix rather than exact wording.
- **Event contracts**: `scan_executables_stream` emits `scan:entry` + `scan:done`; backups emit `backup:progress` and `restore:progress` with `BackupProgressEvent`.
- **Runtime boundary**: Renderer tests should prefer `window.arrancador` and local helpers. Do not add Tauri mocks or Tauri runtime fallbacks.

## Contract test table

### Misc

| Command | Wrapper | Inputs | Output | Error cases |
| --- | --- | --- | --- | --- |
| `greet` | n/a | `name: string` | `string` | None |

### Scan

| Command | Wrapper | Inputs | Output | Error cases |
| --- | --- | --- | --- | --- |
| `scan_executables_stream` | n/a | `dir: string` | `void` + emits `scan:entry`, `scan:done` | None (best-effort scan) |
| `cancel_scan` | n/a | n/a | `void` | None |
| `get_running_processes` | `scanApi.getRunningProcesses` | n/a | `ProcessEntry[]` | None (best-effort snapshot) |

### Games

| Command | Wrapper | Inputs | Output | Error cases |
| --- | --- | --- | --- | --- |
| `get_all_games` | `gamesApi.getAll` | n/a | `Game[]` | DB error |
| `get_game` | `gamesApi.get` | `id: string` | `Game \| null` | DB error |
| `add_game` | `gamesApi.add` | `game: NewGame` | `Game` | DB error, invalid paths |
| `add_games_batch` | `gamesApi.addBatch` | `games: NewGame[]` | `Game[]` | Per-item failures logged, no hard error |
| `update_game` | `gamesApi.update` | `update: UpdateGame` | `Game` | DB error, missing `id` |
| `delete_game` | `gamesApi.delete` | `id: string` | `void` | DB error |
| `toggle_favorite` | `gamesApi.toggleFavorite` | `id: string` | `Game` | DB error |
| `get_favorites` | `gamesApi.getFavorites` | n/a | `Game[]` | DB error |
| `record_game_launch` | `gamesApi.recordLaunch` | `id: string` | `Game` | DB error |
| `search_games` | `gamesApi.search` | `query: string` | `Game[]` | DB error |
| `game_exists_by_path` | `gamesApi.existsByPath` | `exePath: string` | `boolean` | DB error |
| `is_game_installed` | `gamesApi.isInstalled` | `id: string` | `boolean` | DB error |
| `launch_game` | `gamesApi.launch` | `id: string` | `void` | Missing game, invalid exe, spawn failure |
| `get_running_instances` | `gamesApi.getRunningInstances` | `id: string` | `number` | DB error |
| `kill_game_processes` | `gamesApi.killProcesses` | `id: string` | `number` | DB error, process termination failure |
| `resolve_shortcut_target` | `gamesApi.resolveShortcutTarget` | `path: string` | `string` | Invalid shortcut, file not found |

### Metadata (RAWG)

| Command | Wrapper | Inputs | Output | Error cases |
| --- | --- | --- | --- | --- |
| `search_rawg` | `metadataApi.search` | `query: string` | `RawgGame[]` | Network error, API error, parse error |
| `get_rawg_game_details` | `metadataApi.getDetails` | `rawgId: number` | `RawgGameDetails` | Network error, API error, parse error |
| `apply_rawg_metadata` | `metadataApi.apply` | `gameId: string`, `rawgId: number`, `rename: boolean` | `Game` | RAWG error, DB error, missing game |
| `set_rawg_api_key` | `metadataApi.setApiKey` | `key: string` | `void` | DB error |
| `get_rawg_api_key` | `metadataApi.getApiKey` | n/a | `string` | DB error |

### Backups

| Command | Wrapper | Inputs | Output | Error cases |
| --- | --- | --- | --- | --- |
| `check_ludusavi_installed` | `backupApi.checkLudusaviInstalled` | n/a | `boolean` | None (deprecated no-op) |
| `get_ludusavi_executable_path` | `backupApi.getLudusaviPath` | n/a | `string \| null` | None (deprecated no-op) |
| `set_ludusavi_path` | `backupApi.setLudusaviPath` | `path: string` | `void` | None (deprecated no-op) |
| `set_backup_directory` | `backupApi.setBackupDirectory` | `path: string` | `void` | FS error, DB error |
| `get_backup_directory_setting` | `backupApi.getBackupDirectory` | n/a | `string` | DB error |
| `find_game_saves` | `backupApi.findGameSaves` | `gameName: string`, `gameId?: string` | `BackupInfo \| null` | Manifest load error, FS error |
| `create_backup` | `backupApi.create` | `gameId: string`, `gameName: string`, `isAuto: boolean`, `notes?: string` | `Backup` + emits `backup:progress` | Manifest/engine error, FS error, DB error |
| `get_game_backups` | `backupApi.getForGame` | `gameId: string` | `Backup[]` | DB error |
| `restore_backup` | `backupApi.restore` | `backupId: string` | `void` + emits `restore:progress` | Missing backup, engine error, FS error |
| `delete_backup` | `backupApi.delete` | `backupId: string` | `void` | Missing backup, FS error, DB error |
| `should_backup_before_launch` | `backupApi.shouldBackupBeforeLaunch` | `gameId: string` | `boolean` | DB error |
| `check_backup_needed` | `backupApi.checkBackupNeeded` | `gameId: string`, `gameName: string` | `boolean` | Manifest/DB error |
| `check_restore_needed` | `backupApi.checkRestoreNeeded` | `gameId: string`, `gameName: string` | `RestoreCheck` | Manifest/DB error |
| `get_backup_settings` | `backupApi.getSettings` | n/a | `Record<string, string>` | DB error |
| `update_backup_settings` | `backupApi.updateSettings` | `settings: Record<string, string>` | `void` | DB error |

### Settings

| Command | Wrapper | Inputs | Output | Error cases |
| --- | --- | --- | --- | --- |
| `get_all_settings` | `settingsApi.getAll` | n/a | `AppSettings` | DB error |
| `update_settings` | `settingsApi.update` | `settings: AppSettings` | `void` | DB error |
| `get_setting` | `settingsApi.get` | `key: string` | `string \| null` | DB error |
| `set_setting` | `settingsApi.set` | `key: string`, `value: string` | `void` | DB error |
| `add_scan_directory` | `settingsApi.addScanDirectory` | `path: string` | `void` | DB error, FS error |
| `get_scan_directories` | `settingsApi.getScanDirectories` | n/a | `string[]` | DB error |
| `remove_scan_directory` | `settingsApi.removeScanDirectory` | `path: string` | `void` | DB error |

### Stats

| Command | Wrapper | Inputs | Output | Error cases |
| --- | --- | --- | --- | --- |
| `get_playtime_stats` | `statsApi.getPlaytimeStats` | `start?: string`, `end?: string` | `PlaytimeStats` | DB error, invalid date range |

### System

| Command | Wrapper | Inputs | Output | Error cases |
| --- | --- | --- | --- | --- |
| `get_system_info` | `systemApi.getInfo` | n/a | `SystemInfo` | None |
| `test_disk_speed` | `systemApi.testDiskSpeed` | `mountPoint: string` | `DiskSpeedResult` | Invalid mount, IO/permission error |
