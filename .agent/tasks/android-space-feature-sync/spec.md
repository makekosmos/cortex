# Task: android-space-feature-sync

## Original Task Statement

Bring Android (Kotlin/Compose) Delphi to feature parity with TS/Electron on three space-related features:
1. Per-space Room DB isolation (each space = separate SQLite database file)
2. Space naming/renaming
3. Empty trash permanently (delete trashed todos from DB)
4. Store clears on space switch (no stale data)

## Current State

- **DatabaseModule.kt** provides a single `@Singleton` Room `DelphiDatabase` built at `"delphi.db"` (default Android databases path). All spaces share this one DB.
- **SpaceManager.kt** already has `deriveSpaceId(code)` returning `SHA-256(normalized_code)[:16 hex]`, `SavedSpace` data class with `name` field, `saveSpaceToList`, `getSavedSpaces`, and `removeSpaceFromList`. The `name` field exists in persistence but is currently set to the formatted code (not a human-readable label) and is never editable by the user.
- **TodoDao.kt** has `getTrash()` returning `Flow<List<TodoItem>>` with `WHERE isTrashed = 1`, but no `DELETE` query for trashed items.
- **SmartListViewModel** injects `TodoDao` directly. All ViewModels get the single Room DB via Hilt DI.
- **TrashScreen.kt** uses `SmartListScaffold` with no "empty trash" action.
- **SpaceSetupViewModel** calls `spaceManager.setActiveSpaceCode()` and `peerManager.start()` on space switch, but never closes/reopens the DB.
- **TS/Electron reference**: `dbSwitchSpace(spaceId)` kills the old sidecar process and starts a new one pointing at `spaces/<spaceId>/delphi.db`. `dbDeleteTrashed()` sends `delete_trashed` operation to sidecar.

---

## Acceptance Criteria

### AC1: Per-space Room DB isolation

- Each space MUST use a separate Room database file, stored at the Android path: `databases/spaces/<spaceId>/delphi.db` where `spaceId = SHA-256(normalizeCode(code)).take(16)`.
- The `DatabaseModule` singleton pattern MUST be replaced with a mechanism that can close the current Room DB instance and open a new one when the active space changes. Approach: introduce a `DatabaseProvider` (or similar) class annotated `@Singleton` that holds a mutable reference to the current `DelphiDatabase`, exposes current DAOs, and provides a `switchTo(spaceId: String)` method.
- On app startup, if an active space code exists in DataStore, the app MUST open the corresponding per-space DB before any DAO access.
- If no active space is set (first launch / after leaving), no DB should be opened until the user creates or joins a space.
- The `DelphiDatabase` Room class itself is unchanged (same entities, version, schema).

### AC2: Space naming/renaming

- `SpaceManager.SavedSpace.name` MUST default to "Новое пространство" when a new space is created (not the formatted code).
- `SpaceManager` MUST expose a `suspend fun renameSpace(code: String, newName: String)` method that updates the name in the saved spaces list.
- The rename UI MUST be accessible from either `SpaceSetupScreen` (saved spaces list in CHOOSE mode) or `SettingsScreen`.
- The space code (XXXX-XXXX-XXXX) MUST remain unchanged when renaming -- name is a display-only label.
- Saved spaces list in `SpaceSetupScreen` SHOULD display the space name (not just the formatted code).

### AC3: Empty trash permanently

- `TodoDao` MUST have a new query: `@Query("DELETE FROM todos WHERE isTrashed = 1") suspend fun deleteTrashed()`.
- A "Очистить корзину" button/action MUST be visible on the trash screen when there are trashed items.
- Tapping the button SHOULD show a confirmation dialog before permanently deleting.
- After deletion, the trash list MUST update reactively (Room Flow will handle this).
- Related checklist items and tag cross-refs for deleted todos SHOULD also be cleaned up (cascade or explicit delete).

### AC4: Store clears on space switch

- When `switchTo(spaceId)` is called on the DB provider, all existing Room Flow subscriptions from the old DB MUST stop emitting stale data.
- ViewModels that hold `StateFlow<List<TodoItem>>` (and similar) MUST reflect the new space's data after the switch.
- Approach: the `DatabaseProvider.switchTo()` method closes the old DB, opens the new one, and signals (via a `StateFlow` or callback) that DAOs have changed. ViewModels should re-subscribe to the new DAO flows. Alternatively, DAO accessors on the provider can be `StateFlow`-based so downstream collectors automatically get the new DB's data.
- No stale data from the previous space may be visible at any point after the switch completes.

---

## Constraints

- MUST use Room (not raw SQLite). Follow existing Room patterns (`DelphiDatabase`, `@Dao`, `@Entity`).
- MUST use Hilt for DI. Follow existing `@Module`, `@InstallIn(SingletonComponent::class)`, `@Provides`, `@Singleton` patterns.
- Kotlin + Jetpack Compose. Follow existing `hiltViewModel()`, `collectAsStateWithLifecycle()` patterns.
- `spaceId` derivation MUST match TS/Electron exactly: `SHA-256` of uppercase normalized code (no dashes), first 16 hex chars.
- All UUIDs MUST be lowercase.
- DB path convention: `databases/spaces/<spaceId>/delphi.db` (relative to app's internal data directory, which Room resolves via `context.getDatabasePath()`).
- `fallbackToDestructiveMigration(dropAllTables = true)` MUST be preserved on Room builder.
- Minimum SDK 28, target SDK 35.

## Non-Goals

- Migrating existing single-DB data to the per-space layout. Users on the current single-DB will effectively start fresh per-space (acceptable for current development stage).
- Sync protocol changes. The sync layer already works per-space via `PeerManager.start(code, ...)`.
- Changes to macOS/Swift or TS/Electron platforms.
- Adding a "clear all data" button (per CLAUDE.md: data deletes only via system settings).
- Virtual scroll / performance optimization for the trash list.

## Assumptions

- The `SavedSpace.name` field already persisted in DataStore JSON is currently the formatted code string. After this change, newly created spaces will get "Новое пространство" as default. Existing saved spaces will retain their old name value (the formatted code), which is acceptable.
- Room's `databaseBuilder` accepts subdirectory paths (e.g., `spaces/<spaceId>/delphi.db`). Room creates parent directories as needed. If not, the implementation must create directories manually via `context.getDatabasePath()` parent mkdir.
- Closing a Room DB via `db.close()` is safe and does not corrupt data, provided no active transactions are in flight. The switch should await completion of any pending writes.
- The `PendingChange` table (used by sync) is also per-space, so it lives inside the per-space DB (already the case since it is in `DelphiDatabase`).

## Key Files to Modify

| File | Change |
|------|--------|
| `di/DatabaseModule.kt` | Replace singleton DB with `DatabaseProvider` that supports switching |
| `data/space/SpaceManager.kt` | Add `renameSpace()`, change default name to "Новое пространство" |
| `data/db/TodoDao.kt` | Add `deleteTrashed()` query |
| `ui/screens/trash/TrashScreen.kt` | Add "Очистить корзину" button with confirmation |
| `ui/screens/trash/TrashViewModel.kt` | Add `emptyTrash()` method |
| `ui/screens/space/SpaceSetupScreen.kt` | Display space names, add rename UI |
| `ui/screens/space/SpaceSetupViewModel.kt` | Wire `renameSpace()`, call DB switch on space activation |
| `ui/screens/settings/SettingsScreen.kt` | Optionally add rename UI |
| `ui/screens/settings/SettingsViewModel.kt` | Wire DB switch on leave-space |
| `ui/navigation/NavGraph.kt` | Possibly trigger DB init on space-code change |
| `ui/screens/SmartListViewModel.kt` | Re-subscribe to DAO flows after DB switch |
| `ui/components/SmartListScaffold.kt` | Support optional action buttons (for trash empty) |

## Verification Plan

1. **AC1 -- DB isolation**: After implementing, inspect the file system (via `adb shell ls /data/data/com.kazui.delphi/databases/spaces/`) to confirm separate DB files exist for different spaces. Switch between two spaces and verify that todos created in space A do not appear in space B.
2. **AC2 -- Rename**: Create a space, rename it from the UI, leave the space, and verify the saved spaces list shows the new name (not the formatted code).
3. **AC3 -- Empty trash**: Trash several todos, navigate to Корзина, tap "Очистить корзину", confirm the dialog, and verify the trash list is empty. Verify that the todos are actually deleted from the DB (not just hidden).
4. **AC4 -- No stale data**: Create todos in space A, switch to space B (empty), verify the todo list is empty immediately (no flash of space A data). Switch back to space A and verify todos reappear.
5. **Build verification**: `./gradlew assembleDebug` compiles without errors.
6. **Unit test feasibility**: If existing unit tests exist, they must still pass (`./gradlew test`).
