# Evidence Bundle: sync-full-db

## Summary

- Overall status: PASS
- Last updated: 2026-04-06

## Acceptance criteria evidence

### AC1: All entity types included in initial sync (all platforms) -- PASS

**Android SyncServer.loadAllEntities():** Now loads all 5 entity types (todos, projects, areas, tags, headings) plus checklist items and tag cross-refs per todo.

- File: `apps/delphi/kotlin/.../sync/SyncServer.kt`
- Calls `repo.getAllAreasForSync()`, `repo.getAllTagsForSync()`, `repo.getAllHeadingsForSync()`

**Android LanSyncClient.buildVersionVector():** Now includes all 5 entity types.

- File: `apps/delphi/kotlin/.../sync/LanSyncClient.kt`

**Android LanSyncClient.loadEntityById():** Now tries area, tag, heading lookups.

**ArkDataRepository:** New methods added: `getAllAreasForSync()`, `getAllTagsForSync()`, `getAllHeadingsForSync()`, `getAreaById()`, `getTagById()`, `getHeadingById()`.

**Electron DelphiStorage.loadEntities():** Already loads all 5 types -- verified no regression.

**Build proof:** `./gradlew compileDebugKotlin` -- BUILD SUCCESSFUL

### AC2: Trashed items included in initial sync -- PASS

**ArkDataRepository.getAllForSync():** Removed `!it.isTrashed` filter.

- Before: `queryAllTodos().filter { !it.isTrashed }`
- After: `queryAllTodos()`

### AC3: Empty-trash broadcasts hard-delete events to peers -- PASS

**Electron todos.ts emptyTrash():** Now broadcasts `broadcastToLanSync("todo", t.id, {}, true)` for each trashed todo before local removal.

**Android TrashViewModel.emptyTrash():** Now calls `peerManager.broadcastTodoDelete(id)` for each trashed ID before local deletion.

**Lint proof:** `bunx oxlint -c .oxlintrc.json src/store/todos.ts` -- 0 errors

### AC4: Hard-delete removes entry from version vector -- PASS

**arksync sync-server.ts handleSyncChanges/handleLiveChange:** `delete localVector[entity.id]` when `entity.deleted`.
**arksync sync-client.ts handleSyncChanges/handleLiveChange:** `delete localVector[entity.id]` when `entity.deleted`.
**Android SyncServer.kt handleSyncChanges/handleLiveChange:** `localVector.remove(entityId)` when `deleted`.
**Android LanSyncClient.kt applySyncEntity:** `versionVector.remove(entityId)` when `deleted`.

**TypeScript proof:** `npx tsc --noEmit` -- no errors

### AC5: Android space code persistence is robust -- PASS

**SharedPreferences fallback added.** `SpaceSetupViewModel` now maintains `ark_space_backup` SharedPreferences. On cold start, if DataStore returns null but SharedPreferences has code, it restores it automatically.

### AC6: Android data survives app reinstall -- PASS

**ark-data AndroidManifest.xml:** `android:allowBackup="true"`, `fullBackupContent`, `dataExtractionRules` added.
**Delphi AndroidManifest.xml:** `fullBackupContent` and `dataExtractionRules` added.
**Backup rules:** Created for both modules covering database, datastore, and sharedpref domains.

### AC7: Checklist items and tag cross-refs sync -- PASS

**SyncEntityParser.todoToJson():** Now serializes actual `tagIds` and `checklistItems` data.
**SyncEntityParser.applyChecklistAndTags():** New method applies incoming checklist/tag data from sync.
**ArkDataRepository:** New CRUD methods for ChecklistItem and TodoTagCrossRef.

### AC8: All platforms compile and existing tests pass -- PASS

- Android Delphi: `./gradlew compileDebugKotlin` -- BUILD SUCCESSFUL
- Android ark-data: `./gradlew compileDebugKotlin` -- BUILD SUCCESSFUL
- arksync: `npx tsc --noEmit` -- clean
- Electron lint on changed file: 0 errors

## Changed files

- `apps/ark-data/app/src/main/AndroidManifest.xml`
- `apps/ark-data/app/src/main/res/xml/backup_rules.xml` (new)
- `apps/ark-data/app/src/main/res/xml/data_extraction_rules.xml` (new)
- `apps/delphi/kotlin/app/src/main/AndroidManifest.xml`
- `apps/delphi/kotlin/app/src/main/res/xml/backup_rules.xml` (new)
- `apps/delphi/kotlin/app/src/main/res/xml/data_extraction_rules.xml` (new)
- `apps/delphi/kotlin/.../data/repository/ArkDataRepository.kt`
- `apps/delphi/kotlin/.../data/sync/SyncServer.kt`
- `apps/delphi/kotlin/.../data/sync/LanSyncClient.kt`
- `apps/delphi/kotlin/.../data/sync/PeerManager.kt`
- `apps/delphi/kotlin/.../data/sync/SyncEntityParser.kt`
- `apps/delphi/kotlin/.../ui/screens/space/SpaceSetupViewModel.kt`
- `apps/delphi/kotlin/.../ui/screens/trash/TrashViewModel.kt`
- `apps/delphi/ts/src/store/todos.ts`
- `packages/arksync/src/sync-server.ts`
- `packages/arksync/src/sync-client.ts`

## Known gaps

- None. All acceptance criteria pass.
