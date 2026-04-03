# ARK P2P Sync — Work Progress

## 1. packages/arksync — Unified P2P Sync Package

Extracted the entire P2P sync infrastructure from Delphi into a shared package `packages/arksync/` (`@arksync/core`).

### Architecture

```
packages/arksync/
├── index.ts          Full barrel (Node.js — SyncServer, SyncClient, crypto, ws)
├── browser.ts        Browser-safe barrel (HLC, protocol types, space codes)
├── src/
│   ├── hlc.ts        Hybrid Logical Clock (conflict resolution)
│   ├── protocol.ts   LAN sync protocol: types, constants, diff, batch, merge
│   ├── peer-protocol.ts  HMAC auth, nonce, mesh ID (Node.js crypto)
│   ├── space.ts      Space codes, IPv4 encoding, QR payloads, deriveSpaceId
│   ├── storage.ts    StorageBackend interface
│   ├── sync-server.ts  WebSocket server (generic, uses StorageBackend)
│   ├── sync-client.ts  WebSocket client (generic, uses StorageBackend)
│   └── node.ts       Node.js-only utils (getOwnAddresses)
```

### StorageBackend Interface

```typescript
interface StorageBackend {
  loadEntities(vector: VersionVector): Promise<SyncEntity[]>;
  applyEntity(entity: SyncEntity): Promise<void>;
  getKv(key: string): Promise<string | null>;
  setKv(key: string, value: string): Promise<void>;
}
```

Each app implements StorageBackend for its own DB (Delphi = Rust sidecar, Android = Room, etc.).

`SyncEntityType = string` — apps define their own entity types. The sync layer doesn't care.

### Browser vs Node.js

- `browser.ts` — safe for Vite renderer (no crypto/ws/os imports)
- `index.ts` — full barrel for Electron main process and tests
- Vite config aliases: renderer → `browser.ts`, electron main → `index.ts`

---

## 2. Delphi TS/Electron Fixes

### Fix: Infinite peer duplicates
- `sync-server.ts`: `getConnectedPeerNames()` now deduplicates by device_id via Map
- `main.ts`: `lan-sync:getStatus` deduplicates across server + client connections by device_id

### Fix: EPERM when deleting space DB
- `main.ts`: `db:deleteSpace` now calls `sidecar.releaseAndReset()` before `rmSync`
- Added retry loop (3 attempts with 500ms delay) for Windows file lock release
- `sidecar.ts`: added `currentDbPath` getter and `releaseAndReset()` method

### Fix: Store not clearing on space switch
- `App.vue`: `activateSpace()` now `await`s `loadAllFromLocalDb()` instead of fire-and-forget
- Clears headings and sets `hydrated=false` during load
- Preserves existing space name/createdAt on re-activation

### Space naming/renaming
- `space-manager.ts`: added `renameSpace(code, newName)` function
- `SpaceSetup.vue`: shows space name + code, inline rename with edit button
- `SettingsPage.vue`: new "Пространства" section with list, rename, delete, active indicator

### Empty trash permanently
- `sidecar/main.rs`: added `DeleteTrashed` operation (`DELETE FROM todos WHERE is_trashed = 1`)
- Full chain: Rust sidecar → `dbDeleteTrashed()` → IPC handler → `localDbDeleteTrashed()` → store `emptyTrash()` → TrashPage UI button with confirmation

### Pre-existing TS errors fixed
- `peer-server.ts`: null check for `addr`
- `hlc.ts`: `toSorted()` → `sort()` (ES2023 compat)
- `vite.config.ts`: removed unsupported `vaporInterop`

---

## 3. Delphi Android (Kotlin) Feature Sync

### Per-space Room DB isolation (AC1)
- `DatabaseModule.kt`: replaced singleton `DelphiDatabase` with `DatabaseProvider`
- Each space = separate Room DB at `spaces/<spaceId>/delphi.db`
- `switchTo(spaceId)`, `close()`, `deleteSpaceDb(spaceId)` methods
- `dbGeneration` StateFlow for reactive DAO re-subscription

### Space naming/renaming (AC2)
- `SpaceManager.kt`: default name "Новое пространство", `renameSpace()` method
- `SpaceSetupScreen.kt`: shows name + code, inline rename via OutlinedTextField

### Empty trash permanently (AC3)
- `TodoDao.kt`: `deleteTrashed()`, `getTrashedIds()`, cascade cleanup queries
- `TrashScreen.kt`: delete-forever icon with AlertDialog confirmation
- `TrashViewModel.kt`: `emptyTrash()` with cascade delete

### Store clears on space switch (AC4)
- All ViewModels migrated to `DatabaseProvider` with `dbGeneration.flatMapLatest`
- All sync classes (LanSyncClient, SyncServer, ArkPeerManager, ArkSyncClient) migrated

---

## 4. P2P Sync TDD Tests

134 tests total (42 new + 92 existing), all passing.

### New test files:
- `space-manager.test.ts` — 19 tests: localStorage CRUD, renameSpace, deriveSpaceId, edge cases
- `sync-server.test.ts` — 10 tests: real WebSocket, handshake, protocol mismatch, peer dedup, broadcast, stop
- `sync-client.test.ts` — 7 tests: mock server, connect, auth, address racing, reconnection, stop
- `sync-flow.test.ts` — 6 tests: SyncServer+SyncClient E2E: empty sync, batch sync, live mode, peer list, HLC conflict resolution

### Test infrastructure:
- Tests use `StorageBackend` mock (in-memory) instead of sidecar mock
- `fileParallelism: false` in vitest.config.ts to prevent port conflicts
- Integration tests use `// @vitest-environment node` for real WebSocket

---

## 5. Delphi → arksync Migration

- `electron/sync-server.ts` and `electron/sync-client.ts` deleted (replaced by arksync)
- `electron/delphi-storage.ts` created — implements StorageBackend via Rust sidecar
- `main.ts` imports SyncServer/SyncClient from `@arksync/core`, passes DelphiStorage
- `lan-protocol.ts`, `hlc.ts`, `peer-protocol.ts` — thin re-exports from arksync
- `space-manager.ts` — re-exports generic functions + Delphi localStorage persistence
- tsconfig paths added for `@arksync/core` resolution
