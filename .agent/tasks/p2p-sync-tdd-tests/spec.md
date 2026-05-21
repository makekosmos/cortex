# Task: p2p-sync-tdd-tests

## Original Task Statement

Add comprehensive test coverage for the Delphi P2P sync system. Existing tests (~1500 lines) cover pure functions (HLC, lan-protocol, peer-protocol). What is missing:

1. Space manager tests (localStorage functions, deriveSpaceId)
2. SyncServer integration tests with WebSocket mocking
3. SyncClient integration tests (address racing, reconnection)
4. Full protocol flow tests (handshake -> sync -> live mode)

---

## Acceptance Criteria

### AC1: Space Manager unit tests

**File:** `apps/delphi/ts/src/services/space/__tests__/space-manager.test.ts`

Test all localStorage-based functions with a mocked `localStorage`:

- `getSpaces()` returns `[]` on empty/missing/corrupt storage.
- `saveSpace()` persists to `delphi.spaces` key; prepends new space; deduplicates by code.
- `removeSpace()` removes matching code and persists the rest.
- `renameSpace()` updates name in-place; returns `false` for unknown code; falls back to formatted code when name is blank.
- `getActiveSpace()` returns `null` when unset; returns stored code otherwise.
- `setActiveSpace(code)` stores the code; `setActiveSpace(null)` removes the key.
- `deriveSpaceId()` produces a 16-hex-char string; matches a known SHA-256 test vector (input: a known Crockford code, expected: first 16 hex chars of `SHA-256(uppercase_raw_code)`).
- Edge cases: empty storage, duplicate codes on `saveSpace`, invalid JSON in storage.

### AC2: SyncServer integration tests

**File:** `apps/delphi/ts/src/services/sync/__tests__/sync-server.test.ts`

Using real WebSocket connections (`ws` library) against a real `SyncServer` instance with the sidecar module fully mocked via `vi.mock('../../electron/sidecar', ...)` (all `db*` functions return stubs):

- Server starts on the default port (or a free port) and accepts WS connections.
- Hello handshake: client sends `hello` -> server responds with a `hello` message containing its `device_id`, `device_name`, `protocol_version`, and `addresses`.
- Protocol version mismatch -> server closes the connection (close code 1002).
- `getConnectedPeerNames()` returns deduplicated names after multiple connections from the same device_id.
- `getConnectedPeerEntries()` returns deduplicated `{ deviceId, deviceName }` entries.
- `connectedPeerCount` accurately reflects authenticated peer count.
- `broadcastLiveChange(entity, excludeDeviceId)` sends `live_change` to all authenticated peers except the excluded one.
- `registerExternalPeer(deviceId, deviceName, addresses)` adds the peer to known peer records.
- `isConnectedTo(deviceId)` returns `true` for authenticated peers, `false` otherwise.
- `stop()` closes all connections and clears the ping interval.

**Mocking surface (sidecar):**

- `dbGetSyncKv` -> returns `null` or stored JSON string (version vector, known peers).
- `dbSetSyncKv` -> stores value in a test-local `Map<string, string>`.
- `dbLoadAll` -> returns `{ todos: [], projects: [], areas: [], tags: [], headings: [] }` (or test-specific data).
- `dbUpsertTodo`, `dbDeleteTodo`, `dbUpsertProject`, `dbDeleteProject`, `dbUpsertArea`, `dbUpsertTag`, `dbUpsertHeading`, `dbDeleteHeading` -> no-op resolving promises.

### AC3: SyncClient integration tests

**File:** `apps/delphi/ts/src/services/sync/__tests__/sync-client.test.ts`

Using a real `ws.WebSocketServer` to simulate a peer server, with sidecar fully mocked:

- Client connects and sends a `hello` message with correct `protocol_version`, `device_id`, `device_name`, `space_id`, `addresses`.
- Client receives `hello` from server and transitions to `authenticated = true`.
- Getters: `isConnected` is `true` after auth; `peerDeviceId` matches the peer record; `peerName` reflects the name from the server's hello.
- `broadcastLiveChange(entity)` sends a `live_change` message to the connected peer.
- `stop()` disconnects gracefully (close code 1000), clears timers, and does not attempt reconnect.
- Address racing: given a `PeerRecord` with multiple addresses (one valid, others unreachable), the client connects via the first successful address and closes the losers.
- Reconnection with exponential backoff: after disconnect, verify `scheduleReconnect` fires with increasing delay (use `vi.useFakeTimers()` and `vi.advanceTimersByTime()`). Confirm `reconnectDelay` grows (factor 1.5) up to `RECONNECT_MAX_MS` (30000).

### AC4: Full protocol flow tests (end-to-end sync)

**File:** `apps/delphi/ts/src/services/sync/__tests__/sync-flow.test.ts`

`SyncServer` + `SyncClient` connected together, sidecar mocked with in-memory storage for both sides (separate `Map<string, string>` for version vectors, separate entity arrays):

- **Empty <-> empty:** no data exchanged, both sides send empty `sync_changes` with `is_last: true`, both enter live mode.
- **Server has data, client empty:** server sends all entities via `sync_changes` batches; client receives and applies them; client sends `sync_ack` for each batch.
- **Both have data, partial overlap:** only missing/newer entities are exchanged in each direction.
- **Live mode:** after initial sync completes, a local mutation broadcast as `live_change` arrives at the other side with a `live_ack` response.
- **Peer list exchange:** after hello, both sides send `peer_list`; verify the receiver merges new peers into its known list.
- **HLC conflict resolution:** when both sides have the same entity with different HLCs, the newer HLC wins (LWW). The older version is not applied.

### AC5: All existing tests still pass

Running `npx vitest run` (or `bunx vitest run`) from `apps/delphi/ts/` passes all existing and new tests with zero failures.

---

## Constraints

1. **Test file location:** vitest config has `include: ['src/**/*.test.ts']`. All new test files must live under `apps/delphi/ts/src/` even though the code under test is in `electron/`. Use relative import paths like `../../../../electron/sync-server` from test files.
2. **Sidecar mocking:** `electron/sidecar.ts` imports `child_process` and `electron` (Node/Electron-only). It must be mocked via `vi.mock()` at the module level in every test file that imports SyncServer or SyncClient.
3. **WebSocket approach:** Use real `ws` WebSocket connections for AC2/AC3/AC4 (the `ws` library works in Node, and vitest runs these in Node). Do not use jsdom WebSocket.
4. **Fake timers:** Use `vi.useFakeTimers()` only for reconnection/backoff tests (AC3). Real timers for WebSocket tests to avoid interfering with `ws` event loop.
5. **Port conflicts:** Each test suite that starts a real server should use a unique port (e.g., `LAN_SYNC_PORT + offset` or port 0 for OS-assigned) to avoid conflicts with parallel test runs.
6. **No production code changes:** Tests must work against the current production code. No modifications to `sync-server.ts`, `sync-client.ts`, `sidecar.ts`, or `space-manager.ts`.
7. **vitest environment:** Config specifies `environment: 'jsdom'`. Space manager tests (AC1) rely on `localStorage` from jsdom. Sync tests (AC2-AC4) need Node `ws` -- jsdom environment still provides Node globals, so `ws` should work. If not, per-file `// @vitest-environment node` override is acceptable.

## Non-Goals

- No changes to production/source code.
- No Playwright or E2E browser tests.
- No testing of the Rust sidecar binary itself.
- No testing of Electron IPC layer (`main.ts` handlers).
- No testing of Vue components or Pinia stores.
- No testing of mDNS/Bonjour peer discovery.
- No coverage threshold enforcement (just passing tests).

## Assumptions

1. The `ws` npm package is already installed as a dependency (it is -- SyncServer/SyncClient import it).
2. `crypto.subtle.digest` is available in the jsdom vitest environment (Node 18+ provides it via `globalThis.crypto`). If not, a polyfill or `// @vitest-environment node` is acceptable for AC1's `deriveSpaceId` test.
3. SyncServer binds to `LAN_SYNC_PORT` (21531) by default. Tests may need to use different ports to avoid conflicts; this can be done by temporarily mocking `LAN_SYNC_PORT` or passing a port option. The current `start()` method hardcodes `LAN_SYNC_PORT` in the `WebSocketServer` constructor -- the test may need to work around this (e.g., start server on 21531 if no conflict, or mock the constant).
4. The `hello` response from SyncServer is a `HelloMessage` (type `"hello"`), not a separate `"hello_ack"` type. The protocol uses `hello` in both directions.
5. Both SyncServer and SyncClient share the same `VERSION_VECTOR_KEY = 'lan_sync.version_vector'` for persistence. Tests must provide separate mock storage per side to avoid cross-contamination (this is the known bug documented in memory).

## Verification Plan

1. Run `npx vitest run` from `apps/delphi/ts/` and confirm all tests pass (exit code 0).
2. Confirm each new test file exists at the specified path.
3. Confirm each AC's test cases are present by inspecting `describe`/`it` blocks.
4. Confirm no production files were modified (`git diff --name-only` shows only new files under `src/services/`).
5. Confirm sidecar is mocked (grep for `vi.mock` referencing sidecar in AC2/AC3/AC4 test files).
