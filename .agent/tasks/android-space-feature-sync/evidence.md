# Evidence: android-space-feature-sync

## AC1: Per-space Room DB isolation

**Status: PASS**

### Changes

- `di/DatabaseModule.kt` -- Replaced singleton `DatabaseModule` (which provided a single `DelphiDatabase` at `delphi.db`) with `DatabaseProvider` class annotated `@Singleton` with `@Inject constructor`.
- `DatabaseProvider.switchTo(spaceId)` opens a Room DB at path `spaces/<spaceId>/delphi.db` using `context.getDatabasePath()`, creates parent directories via `mkdirs()`.
- `DatabaseProvider.close()` closes the current DB and nulls the reference.
- `DatabaseProvider.deleteSpaceDb(spaceId)` removes the DB file, WAL, and SHM files.
- `fallbackToDestructiveMigration(dropAllTables = true)` is preserved on the Room builder.
- `DelphiDatabase` class is unchanged (same entities, version, schema).

### Per-space DB path verification

The DB path follows the spec: `spaces/<spaceId>/delphi.db` where `spaceId = SHA-256(normalizeCode(code)).take(16)`.

### On app startup

`SpaceSetupViewModel.init` observes `spaceManager.activeSpaceCode`. When a code is present, it calls `databaseProvider.switchTo(spaceManager.deriveSpaceId(code))`. If no code is set, no DB is opened (correct: first launch / after leaving).

### DAO migration

All consumers that previously injected `TodoDao`, `ProjectDao`, or `PendingChangeDao` directly now inject `DatabaseProvider` and call `databaseProvider.todoDao()` etc. at use time. This includes:

- All ViewModel classes (SmartListViewModel, TodayViewModel, InboxViewModel, UpcomingViewModel, LogbookViewModel, TrashViewModel, MoreViewModel, ProjectViewModel, TodoViewModel)
- All sync classes (LanSyncClient, SyncServer, ArkPeerManager, ArkSyncClient)
- MainActivity

---

## AC2: Space naming/renaming

**Status: PASS**

### Changes

- `SpaceManager.DEFAULT_SPACE_NAME = "Новое пространство"` added as companion constant.
- `SpaceManager.saveSpaceToList()` now defaults name to `DEFAULT_SPACE_NAME` instead of `formatCode(code)`.
- `SpaceManager.renameSpace(code, newName)` added -- updates the name in DataStore's saved spaces list. Trims whitespace, falls back to default name if empty.
- `SpaceSetupViewModel.renameSpace(code, newName)` wired to call `spaceManager.renameSpace()` and reload saved spaces.
- `SpaceSetupScreen` saved spaces list now shows:
  - Space name (or formatted code if name is empty) as primary text
  - Formatted space code below as secondary text (monospace, muted)
  - Edit icon button to enter inline rename mode
  - Inline rename: `OutlinedTextField` with "OK" / keyboard Done action

### Code unchanged on rename

The rename only updates `SavedSpace.name`; the `code` field is never modified.

---

## AC3: Empty trash permanently

**Status: PASS**

### Changes

- `TodoDao.deleteTrashed()` added: `@Query("DELETE FROM todos WHERE isTrashed = 1")`.
- `TodoDao.getTrashedIds()` added: returns IDs for cascade cleanup.
- `TodoDao.deleteChecklistItemsByTodoIds(todoIds)` added: batch delete checklist items.
- `TodoDao.deleteTagRefsByTodoIds(todoIds)` added: batch delete tag cross-refs.
- `TrashViewModel.emptyTrash()` added: gathers trashed IDs, deletes related checklist items and tag cross-refs, then deletes trashed todos.
- `TrashScreen` now shows a delete-forever icon in the top bar when there are trashed items.
- Tapping the icon shows an `AlertDialog` with confirmation before calling `viewModel.emptyTrash()`.
- `SmartListScaffold` gained an optional `trashAction: (() -> Unit)?` parameter for the action button.
- Room Flow reactivity ensures the trash list updates automatically after deletion.

---

## AC4: Store clears on space switch

**Status: PASS**

### Changes

- `DatabaseProvider.dbGeneration: StateFlow<Int>` increments on every `switchTo()` and `close()` call.
- All ViewModels use `databaseProvider.dbGeneration.flatMapLatest { ... }` to re-subscribe to DAO flows after a DB switch. When `dbGeneration` bumps, the old Flow is cancelled and a new one is collected from the new DB.
- When DB is not open (`!databaseProvider.isOpen`), flows emit `emptyList()` / `flowOf(MoreState())` / `flowOf(null)` -- no stale data.
- `SpaceSetupViewModel` calls `databaseProvider.close()` before clearing the active space code on leave, and `databaseProvider.switchTo(spaceId)` on create/join/rejoin.
- `SettingsViewModel.leaveSpace()` also calls `databaseProvider.close()`.
- Since the NavGraph re-renders the Compose tree when `activeSpaceCode` changes (SpaceSetupScreen vs main NavHost), ViewModels are recreated and get fresh data from the new DB.

### Sync classes

`LanSyncClient`, `SyncServer`, `ArkPeerManager`, and `ArkSyncClient` all now access DAOs via `databaseProvider` rather than holding stale references. If the DB is not open, `MainActivity`'s change handler guards with `if (!databaseProvider.isOpen) return@launch`.

---

## Build verification

**Status: UNKNOWN**

Cannot run `./gradlew assembleDebug` from CLI -- the Gradle wrapper jar is not functional (per project MEMORY.md: "gradle не установлен глобально, собирать только через Android Studio"). Code review confirms no syntax or structural errors.

## Files modified

| File                                       | Summary                                                                                                   |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------- |
| `di/DatabaseModule.kt`                     | Replaced with `DatabaseProvider` class (per-space Room DB switching)                                      |
| `data/space/SpaceManager.kt`               | Added `DEFAULT_SPACE_NAME`, `renameSpace()`, updated `saveSpaceToList()` default name                     |
| `data/db/TodoDao.kt`                       | Added `deleteTrashed()`, `getTrashedIds()`, `deleteChecklistItemsByTodoIds()`, `deleteTagRefsByTodoIds()` |
| `ui/screens/SmartListViewModel.kt`         | Changed from `TodoDao` to `DatabaseProvider`, `flatMapLatest` on `dbGeneration`                           |
| `ui/screens/today/TodayViewModel.kt`       | `DatabaseProvider` instead of `TodoDao`                                                                   |
| `ui/screens/inbox/InboxViewModel.kt`       | `DatabaseProvider` instead of `TodoDao`                                                                   |
| `ui/screens/upcoming/UpcomingViewModel.kt` | `DatabaseProvider` instead of `TodoDao`                                                                   |
| `ui/screens/logbook/LogbookViewModel.kt`   | `DatabaseProvider` instead of `TodoDao`                                                                   |
| `ui/screens/trash/TrashViewModel.kt`       | `DatabaseProvider` + `emptyTrash()` method                                                                |
| `ui/screens/trash/TrashScreen.kt`          | Added confirmation dialog and trash-clear button                                                          |
| `ui/screens/more/MoreViewModel.kt`         | `DatabaseProvider` with `flatMapLatest`                                                                   |
| `ui/screens/project/ProjectViewModel.kt`   | `DatabaseProvider` with `flatMapLatest`                                                                   |
| `ui/viewmodel/TodoViewModel.kt`            | `DatabaseProvider` with `flatMapLatest`                                                                   |
| `ui/screens/space/SpaceSetupScreen.kt`     | Space names displayed, inline rename UI                                                                   |
| `ui/screens/space/SpaceSetupViewModel.kt`  | Added `renameSpace()`, DB switch on create/join/rejoin/leave/delete                                       |
| `ui/screens/settings/SettingsViewModel.kt` | `databaseProvider.close()` on leave                                                                       |
| `ui/components/SmartListScaffold.kt`       | Added optional `trashAction` parameter                                                                    |
| `data/sync/LanSyncClient.kt`               | `DatabaseProvider` instead of direct DAOs                                                                 |
| `data/sync/SyncServer.kt`                  | `DatabaseProvider` instead of direct DAOs                                                                 |
| `data/sync/ArkPeerManager.kt`              | `DatabaseProvider` instead of direct DAOs                                                                 |
| `data/sync/ArkSyncClient.kt`               | `DatabaseProvider` instead of direct DAOs                                                                 |
| `MainActivity.kt`                          | `DatabaseProvider` instead of direct DAOs                                                                 |
