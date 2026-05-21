# Evidence: p2p-sync-tdd-tests

## Test Run

Command: `cd apps/delphi/ts && npx vitest run`
Result: **12 test files, 134 tests, 0 failures** (exit code 0)
Duration: ~31s

## AC1: Space Manager unit tests -- PASS

**File:** `apps/delphi/ts/src/services/space/__tests__/space-manager.test.ts`

19 test cases covering:

- `getSpaces()`: empty storage, invalid JSON, stored spaces
- `saveSpace()`: persistence, prepending, deduplication by code
- `removeSpace()`: removal + persistence, no-op for unknown code
- `renameSpace()`: in-place update, false for unknown, fallback to formatted code
- `getActiveSpace()`: null when unset, returns stored code
- `setActiveSpace()`: stores code, removes key on null
- `deriveSpaceId()`: 16-hex output, known SHA-256 vector, normalization
- Edge cases: empty string in storage, corrupt storage recovery

## AC2: SyncServer integration tests -- PASS

**File:** `apps/delphi/ts/src/services/sync/__tests__/sync-server.test.ts`

10 test cases using real WebSocket connections:

- Server starts and accepts WS connections
- Hello handshake returns correct device_id, device_name, protocol_version, addresses
- Protocol version mismatch closes with code 1002
- `getConnectedPeerNames()` deduplicates by device_id
- `getConnectedPeerEntries()` deduplicates entries
- `connectedPeerCount` reflects authenticated count
- `broadcastLiveChange()` sends to authenticated peers, excludes specified device
- `registerExternalPeer()` adds to known records
- `isConnectedTo()` returns true/false correctly
- `stop()` closes connections and clears state

Sidecar mocked via `vi.mock('../../../../electron/sidecar')`.

## AC3: SyncClient integration tests -- PASS

**File:** `apps/delphi/ts/src/services/sync/__tests__/sync-client.test.ts`

7 test cases using a real ws.WebSocketServer as mock peer:

- Client connects and sends hello with correct fields
- Transitions to authenticated after receiving server hello
- `peerDeviceId` and `peerName` reflect peer record/server hello
- `broadcastLiveChange()` sends live_change to peer
- `stop()` disconnects gracefully, no reconnect
- Address racing: connects via first successful address from multiple
- Reconnection: verifies client reconnects after server disconnect

Sidecar mocked via `vi.mock('../../../../electron/sidecar')`.
Uses `// @vitest-environment node`.

## AC4: Full protocol flow tests -- PASS

**File:** `apps/delphi/ts/src/services/sync/__tests__/sync-flow.test.ts`

6 test cases with real SyncServer + SyncClient:

- Empty-empty sync: no data exchanged
- Server has data: entities synced, version vector populated
- Both have data: version vectors exchanged and merged
- Live mode: bidirectional live_change delivery + persistence
- Peer list exchange: external peers merged into client's known list
- HLC conflict resolution: newer HLC accepted, older rejected

Shared sidecar mock (known bug: shared VERSION_VECTOR_KEY). Tests designed to work with this constraint.
Uses `// @vitest-environment node`.

## AC5: All existing tests still pass -- PASS

Full test run: 134 tests across 12 files, 0 failures.
Pre-existing test files (8 files, 92 tests) unmodified and passing.
New test files (4 files, 42 tests) all passing.

## Files Changed

New files:

- `apps/delphi/ts/src/services/space/__tests__/space-manager.test.ts`
- `apps/delphi/ts/src/services/sync/__tests__/sync-server.test.ts`
- `apps/delphi/ts/src/services/sync/__tests__/sync-client.test.ts`
- `apps/delphi/ts/src/services/sync/__tests__/sync-flow.test.ts`

Modified (test infrastructure only):

- `apps/delphi/ts/vitest.config.ts` -- added `fileParallelism: false` to prevent port conflicts between integration tests

No production files modified.
