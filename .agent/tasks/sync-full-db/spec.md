# Task Spec: sync-full-db

## Metadata
- Task ID: sync-full-db
- Created: 2026-04-04
- Repo root: /Users/kirill/Documents/projects/kosmos

## Guidance sources
- `/CLAUDE.md` (repo task proof loop)
- `/apps/delphi/CLAUDE.md` (P2P sync architecture, data model, all platforms)
- `/apps/delphi/kotlin/CLAUDE.md` (Android specifics, Ark Data ContentProvider)

## Original task statement

Four user-reported problems:

1. Incomplete sync: when a space already has data, a new device joining does not receive existing tasks.
2. "Empty trash" (permanent delete) does not sync: other devices still show the deleted tasks.
3. Android space code sometimes lost after app restart -- SpaceSetupScreen shown instead of main UI.
4. Data lost on Android app reinstall.

Requirements from user:
- ALL devices in a space MUST have IDENTICAL DB -- not just todos, but all entities (projects, areas, tags, headings, checklist items, notes).
- Permanent delete = hard delete, must propagate to all devices.
- Data must survive app reinstall on Android (Auto Backup or External Storage).
- Data removal only on full app uninstall + cache clear confirmation.

---

## Root-cause analysis (from code reading)

### Problem 1: Incomplete initial sync for new devices

**Android `loadAllEntities` (SyncServer.kt:658-688) only loads `todos` and `projects`.** It does NOT load areas, tags, or headings. Compare with Electron's `DelphiStorage.loadEntities()` (delphi-storage.ts:31-81) which correctly loads all five entity types.

Additionally, **`ArkDataRepository.getAllForSync()` (line 128-130) filters out trashed items** (`!it.isTrashed`). This means trashed-but-not-permanently-deleted items are never sent during initial sync, so a new device will never see items that are in the trash on the source device.

The `LanSyncClient.buildVersionVector()` (line 607-633) also only loads todos and projects, missing areas/tags/headings.

The `LanSyncClient.loadEntityById()` (line 661-682) only tries todo and project, so when responding to a version_vector diff request, it cannot provide area/tag/heading entities.

### Problem 2: Empty trash does not sync

**Electron `emptyTrash()` (todos.ts:713-721)** deletes from local store and calls `localDbDeleteTrashed()` but does NOT call `broadcastToLanSync("todo", id, {}, true)` for each deleted item. The hard delete is purely local.

**Android `TrashViewModel.emptyTrash()` (TrashViewModel.kt:21-31)** performs `repo.deleteTrashed()` but does NOT call `peerManager.broadcastTodoDelete(id)` for each deleted item. Again, purely local.

The sync protocol already supports `deleted: true` flag on SyncEntity (protocol.ts:36) and both `DelphiStorage.applyEntity()` and `SyncServer.applySyncEntity()` handle it correctly. The problem is purely that the empty-trash operations do not emit delete events.

### Problem 3: Android space code lost after restart

`SpaceManager.activeSpaceCode` is a Flow backed by DataStore key `ark.space.activeCode`. The `SpaceSetupViewModel` reads this at startup. If DataStore read returns null transiently (e.g., DataStore file corruption, race condition during cold start), SpaceSetupScreen appears.

Need to investigate: potential race between DataStore initialization and NavGraph composition, or DataStore file getting cleared during Android system data management.

### Problem 4: Data lost on Android reinstall

Android clears internal storage on reinstall by default. Ark-data ContentProvider stores its Room database in internal storage. No `android:allowBackup` / `android:fullBackupContent` / `android:dataExtractionRules` configuration reviewed yet.

---

## Acceptance criteria

### AC1: All entity types included in initial sync (all platforms)

Android `SyncServer.loadAllEntities()` and `LanSyncClient.buildVersionVector()` and `LanSyncClient.loadEntityById()` MUST load and emit all five entity types: todo, project, area, tag, heading.

Android `ArkDataRepository` MUST expose `getAllAreasForSync()`, `getAllTagsForSync()`, `getAllHeadingsForSync()` methods (or equivalent).

Electron `DelphiStorage.loadEntities()` already handles all five types -- verify no regression.

### AC2: Trashed items included in initial sync

`ArkDataRepository.getAllForSync()` MUST NOT filter out trashed items (remove `!it.isTrashed` filter). Trashed items are part of the database state and must sync so that all devices show the same trash contents.

### AC3: Empty-trash broadcasts hard-delete events to peers

**Electron:** `emptyTrash()` in `todos.ts` MUST call `broadcastToLanSync("todo", id, {}, true)` (with `deleted=true`) for each trashed todo before removing from local store.

**Android:** `TrashViewModel.emptyTrash()` MUST call `peerManager.broadcastTodoDelete(id)` for each trashed todo ID before deleting locally.

Both platforms: the receiving side already handles `deleted: true` correctly via `applyEntity` / `applySyncEntity` -- verify no regression.

### AC4: Hard-delete removes entry from version vector

When a hard-delete entity arrives (SyncEntity with `deleted: true`), after removing the row from the database, the entity ID MUST also be removed from the persisted version vector. Otherwise, the version vector will reference a non-existent entity and future syncs may malfunction.

Applies to: `SyncServer.handleSyncChanges()`, `SyncServer.handleLiveChange()`, `SyncClient.handleSyncChanges()`, `SyncClient.handleLiveChange()` in `packages/arksync/`, and their Android counterparts in `SyncServer.kt` and `LanSyncClient.kt`.

### AC5: Android space code persistence is robust

Investigate and harden the DataStore read path in `SpaceSetupViewModel` so that a transient DataStore read failure does not cause the app to show SpaceSetupScreen. Possible approaches:
- Add retry/fallback when DataStore returns null on cold start.
- Wait for DataStore to be fully initialized before rendering NavGraph decision.
- Persist space code redundantly (SharedPreferences fallback) if DataStore is unreliable.

After fix: the active space code MUST survive app restart, device reboot, and process death.

### AC6: Android data survives app reinstall

Configure Android Auto Backup (or equivalent) so that Ark-data ContentProvider database and Delphi DataStore preferences are included in backup/restore. Alternatively, store the database in external/scoped storage that survives reinstall.

After fix: reinstalling Delphi (without clearing cache) MUST preserve existing todos, projects, and sync state.

### AC7: Checklist items and tag cross-refs sync

The data model includes ChecklistItem and tag-todo cross-references. These MUST be included in sync entities. Currently, `SyncEntityParser.todoToJson()` emits empty arrays for `tagIds` and `checklistItems`. These should contain actual data so they sync correctly.

### AC8: All platforms compile and existing tests pass

Changes MUST NOT break builds on any platform:
- `bun run build` (TS/Electron)
- `./gradlew assembleDebug` (Android/Kotlin)
- Existing unit tests pass: `bun run test`, `./gradlew test`

---

## Constraints

- All three platforms (Electron/TS, Android/Kotlin, macOS/Swift) implement the same protocol version (currently 1). Protocol changes must be backward-compatible or version must be bumped on all platforms simultaneously.
- Do not change the SyncEntity wire format in a breaking way -- the `deleted: boolean` field already exists and is sufficient for hard deletes.
- Version vector keys are entity IDs (UUIDs). The version vector can grow large over time; this is an existing design limitation, not in scope to fix.
- The macOS/Swift platform exists but is out of scope for implementation in this task (no user-reported issues on macOS). However, protocol-level changes in `packages/arksync/` affect all platforms.
- Android Ark-data is a separate APK (`com.kepler.ark.data`). Its ContentProvider API is the interface boundary -- changes to its database schema or backup behavior may require changes in the ark-data project.

## Non-goals

- Conflict resolution improvements (current HLC-based LWW is acceptable).
- Sync performance optimization (batching, compression).
- End-to-end encryption of sync traffic.
- Cloud/relay sync (VPS-based). Only LAN P2P sync is in scope.
- macOS/Swift platform changes (unless protocol-level changes require it).
- Recurrence data sync (not currently a sync entity type; can be a follow-up task).
- Notes entity sync (mentioned in data model but not currently implemented as sync entity).
- UI changes beyond what is needed for the fixes.

## Assumptions

- A1: The existing `deleted: true` flag in SyncEntity is sufficient for hard-delete propagation. No new message type is needed.
- A2: Android Auto Backup is the appropriate mechanism for data persistence across reinstalls (vs. external storage or SAF).
- A3: The space code instability on Android is caused by a DataStore read race, not by intentional clearing of preferences.
- A4: Checklist items and tag cross-refs are currently NOT synced (empty arrays in SyncEntityParser). Full sync of these is required by the user's "identical DB" requirement but may be deferred to a sub-task if complexity is too high.

---

## Verification plan

### Build
- `cd /Users/kirill/Documents/projects/kosmos && bun run build` -- Electron/TS compiles
- `cd /Users/kirill/Documents/projects/kosmos/apps/delphi/kotlin && ./gradlew assembleDebug` -- Android compiles

### Unit tests
- `cd /Users/kirill/Documents/projects/kosmos && bun run test` -- existing tests pass
- `cd /Users/kirill/Documents/projects/kosmos/apps/delphi/kotlin && ./gradlew test` -- existing tests pass

### Code review checks
- Verify `loadAllEntities` on Android includes all 5 entity types
- Verify `getAllForSync` does not filter trashed items
- Verify `emptyTrash` on both Electron and Android broadcasts delete events
- Verify version vector entry removal on hard-delete receipt
- Verify SpaceManager DataStore read is hardened
- Verify Android backup configuration

### Manual checks
- Create space on Electron with todos, projects, areas, tags, headings
- Join from Android -- verify all entities arrive
- Empty trash on Electron -- verify Android removes same items
- Empty trash on Android -- verify Electron removes same items
- Restart Android app -- verify space code persists
- (If Auto Backup configured) Reinstall Android app -- verify data persists
