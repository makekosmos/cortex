/**
 * Integration-style tests that reproduce the Electron → Android sync bug.
 *
 * ROOT CAUSE: Android's LanSyncClient rebuilds its version vector from scratch
 * on every reconnect, assigning HLC = Instant.now() to ALL entities. This makes
 * Android's HLCs always "newer" than Electron's, causing:
 *   - Android rejects live_change updates from Electron (HLC comparison fails)
 *   - On reconnect, Android sends its old data back to Electron as "newer"
 *   - Electron overwrites its local completions/deletions with Android's stale data
 *
 * This explains why CREATION works (new entities don't exist in Android's vector)
 * but COMPLETION/DELETION from Electron doesn't (entities already in Android's vector
 * with a freshly-generated "newer" HLC).
 */

import { describe, expect, it } from "vitest";

import {
  type LiveChangeMessage,
  type SyncEntity,
  type VersionVector,
  computeVectorDiff,
  deserializeMessage,
  isNewerHlc,
  serializeMessage,
} from "../lan-protocol";

import { HLC } from "../hlc";

// ---------------------------------------------------------------------------

// Helpers

// ---------------------------------------------------------------------------

function makeTodoEntity(
  id: string,

  hlc: string,

  overrides: Record<string, unknown> = {},
): SyncEntity {
  return {
    type: "todo",

    id,

    data: {
      id,

      title: "Test task",

      isCompleted: false,

      isToday: false,

      isEvening: false,

      isSomeday: false,

      isCancelled: false,

      isTrashed: false,

      priority: 0,

      sortOrder: 0,

      createdAt: "2026-03-28T10:00:00.000Z",

      tagIds: [],

      checklistItems: [],

      ...overrides,
    },

    hlc,
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ---------------------------------------------------------------------------

// Test: live_change message preserves isCompleted

// ---------------------------------------------------------------------------

describe("live_change message format for completion", () => {
  it("preserves isCompleted=true in the entity data through serialization", () => {
    const entity = makeTodoEntity(
      "todo-1",

      "2026-03-28T14:30:00.000Z:000000:electron-abc",

      { isCompleted: true, completedAt: "2026-03-28T14:30:00.000Z" },
    );

    const msg: LiveChangeMessage = {
      type: "live_change",

      change_id: "test-change-1",

      entity,
    };

    // Simulate WS serialization round-trip

    const serialized = serializeMessage(msg);

    const deserialized = deserializeMessage(serialized) as LiveChangeMessage;

    expect(deserialized.entity.data.isCompleted).toBe(true);

    expect(deserialized.entity.data.completedAt).toBe(
      "2026-03-28T14:30:00.000Z",
    );
  });

  it("preserves deleted flag through serialization", () => {
    const entity: SyncEntity = {
      type: "todo",

      id: "todo-1",

      data: {},

      hlc: "2026-03-28T14:30:00.000Z:000000:electron-abc",

      deleted: true,
    };

    const msg: LiveChangeMessage = {
      type: "live_change",

      change_id: "test-change-2",

      entity,
    };

    const serialized = serializeMessage(msg);

    const deserialized = deserializeMessage(serialized) as LiveChangeMessage;

    expect(deserialized.entity.deleted).toBe(true);
  });
});

// ---------------------------------------------------------------------------

// Test: HLC-based conflict resolution for the completion scenario

// ---------------------------------------------------------------------------

describe("version vector conflict: Electron completion vs Android reconnect", () => {
  it("server update should be newer than initial creation HLC", async () => {
    // Step 1: Both sides have entity X with creation HLC at T1

    const creationHlc = "2026-03-28T10:00:00.000Z:000000:electron-abc";

    // Step 2: Electron completes the entity, gets new HLC at T2 (later)

    const completionHlc = "2026-03-28T14:30:00.000Z:000000:electron-abc";

    // Step 3: Verify T2 > T1

    expect(isNewerHlc(completionHlc, creationHlc)).toBe(true);

    // Step 4: Client should accept the change

    const clientVector: VersionVector = {
      "todo-1": creationHlc,
    };

    const diff = computeVectorDiff(clientVector, { "todo-1": completionHlc });

    expect(diff.has("todo-1")).toBe(true);
  });

  it("BUG REPRODUCTION: Android regenerating HLCs on reconnect defeats server updates", async () => {
    // This test demonstrates the root cause of the bug.

    // T0: Entity X is created on Electron at 10:00

    const electronCreationHlc = "2026-03-28T10:00:00.000Z:000000:electron-abc";

    // T1: Android connects at 10:01, builds version vector, gets HLC = now()

    const androidReconnectHlc = "2026-03-28T10:01:00.000Z:000000:android-xyz";

    // Android's HLC is NEWER than Electron's creation HLC

    expect(isNewerHlc(androidReconnectHlc, electronCreationHlc)).toBe(true);

    // T2: User completes todo on Electron at 10:02

    const electronCompletionHlc =
      "2026-03-28T10:02:00.000Z:000000:electron-abc";

    // The completion HLC IS newer than Android's reconnect HLC

    expect(isNewerHlc(electronCompletionHlc, androidReconnectHlc)).toBe(true);

    // So live_change during the same session WOULD work

    // T3: Android reconnects at 10:05 (e.g., app was in background)

    // It rebuilds version vector with HLC = now() = 10:05

    const androidSecondReconnectHlc =
      "2026-03-28T10:05:00.000Z:000000:android-xyz";

    // Now Android's HLC is NEWER than Electron's completion HLC!

    expect(isNewerHlc(androidSecondReconnectHlc, electronCompletionHlc)).toBe(
      true,
    );

    // This means:

    // 1. During version vector exchange, Android's version looks "newer"

    // 2. Android sends its OLD (uncompleted) data to Electron

    // 3. Electron accepts it because Android's HLC > Electron's HLC

    // 4. Completion is LOST

    // The server would NOT send entity X to Android (server thinks Android has newer)

    const serverVector: VersionVector = {
      "todo-1": electronCompletionHlc,
    };

    const androidVector: VersionVector = {
      "todo-1": androidSecondReconnectHlc,
    };

    // Server computes diff: what does Android need from us?

    const androidNeeds = computeVectorDiff(androidVector, serverVector);

    expect(androidNeeds.size).toBe(0); // Server WON'T send completion to Android!

    // Android computes diff: what does server need from us?

    const serverNeeds = computeVectorDiff(serverVector, androidVector);

    expect(serverNeeds.size).toBe(1); // Android WILL send its old data to server!

    expect(serverNeeds.has("todo-1")).toBe(true);
  });

  it("FIX: with persisted version vector, Android reconnect preserves correct HLCs", () => {
    // After fix: Android persists its version vector and doesn't regenerate HLCs.

    // T0: Entity X is created on Electron at 10:00

    const electronCreationHlc = "2026-03-28T10:00:00.000Z:000000:electron-abc";

    // T1: Android connects, receives entity X from initial sync.

    // Android stores the HLC that came from the server (not a freshly generated one).

    const androidStoredHlc = electronCreationHlc; // Same as what server sent

    // T2: User completes todo on Electron at 10:02

    const electronCompletionHlc =
      "2026-03-28T10:02:00.000Z:000000:electron-abc";

    // T3: Android reconnects. It loads its PERSISTED version vector.

    // The stored HLC for entity X is still the original creation HLC.

    const androidVector: VersionVector = {
      "todo-1": androidStoredHlc,
    };

    const serverVector: VersionVector = {
      "todo-1": electronCompletionHlc,
    };

    // Server computes diff: what does Android need?

    const androidNeeds = computeVectorDiff(androidVector, serverVector);

    expect(androidNeeds.size).toBe(1); // Server WILL send the completed version

    expect(androidNeeds.has("todo-1")).toBe(true);

    // Android computes diff: what does server need?

    const serverNeeds = computeVectorDiff(serverVector, androidVector);

    expect(serverNeeds.size).toBe(0); // Android WON'T overwrite server's data
  });
});

// ---------------------------------------------------------------------------

// Test: HLC generation consistency

// ---------------------------------------------------------------------------

describe("HLC generation for version vectors", () => {
  it("HLC.now() generates monotonically increasing values", async () => {
    const hlc1 = HLC.now("device-1");

    await sleep(2);

    const hlc2 = HLC.now("device-1");

    expect(hlc2.compareTo(hlc1)).toBeGreaterThan(0);
  });

  it("entities received from sync should use the received HLC, not a new one", () => {
    // Simulates the correct behavior: when applying a sync entity,

    // store the HLC that came WITH the entity.

    const receivedHlc = "2026-03-28T10:00:00.000Z:000000:electron-abc";

    // Correct behavior: use the received HLC

    const versionVector: VersionVector = {};

    versionVector["todo-1"] = receivedHlc; // Correct: preserve original

    // Later, when server sends an update with a newer HLC:

    const updateHlc = "2026-03-28T14:30:00.000Z:000000:electron-abc";

    expect(isNewerHlc(updateHlc, versionVector["todo-1"])).toBe(true);

    // The update would be accepted -- correct!
  });
});

// ---------------------------------------------------------------------------

// Test: full sync scenario simulation

// ---------------------------------------------------------------------------

describe("full sync scenario: create → complete → reconnect", () => {
  it("simulates the corrected sync flow", () => {
    // SERVER (Electron) state

    const serverVector: VersionVector = {};

    // 1. Create todo on server

    const createHlc = "2026-03-28T10:00:00.000Z:000000:electron";

    serverVector["todo-1"] = createHlc;

    // 2. Android connects, receives entity, stores HLC from server

    const clientVector: VersionVector = {};

    clientVector["todo-1"] = createHlc; // Uses received HLC, not regenerated

    // 3. Complete on server

    const completeHlc = "2026-03-28T14:00:00.000Z:000000:electron";

    serverVector["todo-1"] = completeHlc;

    // 4. Live change sent to Android

    // Android checks: is server's HLC newer than stored?

    expect(isNewerHlc(completeHlc, clientVector["todo-1"])).toBe(true);

    // YES - so Android accepts the change

    // 5. Android updates its vector

    clientVector["todo-1"] = completeHlc;

    // 6. Android reconnects later

    // With persisted vector, both sides agree

    const androidNeeds = computeVectorDiff(clientVector, serverVector);

    expect(androidNeeds.size).toBe(0); // No diff - both in sync

    const serverNeeds = computeVectorDiff(serverVector, clientVector);

    expect(serverNeeds.size).toBe(0); // No diff - both in sync
  });

  it("simulates reconnect after missed live_change (Android was offline)", () => {
    // SERVER state

    const serverVector: VersionVector = {};

    // 1. Create todo

    const createHlc = "2026-03-28T10:00:00.000Z:000000:electron";

    serverVector["todo-1"] = createHlc;

    // 2. Android connects, syncs, stores HLC

    const clientVector: VersionVector = {};

    clientVector["todo-1"] = createHlc;

    // 3. Android disconnects (goes to background)

    // 4. Server completes the todo while Android is disconnected

    const completeHlc = "2026-03-28T14:00:00.000Z:000000:electron";

    serverVector["todo-1"] = completeHlc;

    // 5. Android reconnects (with FIX: persisted vector)

    // Client still has the old HLC from step 2

    // 6. Version vector exchange

    const androidNeeds = computeVectorDiff(clientVector, serverVector);

    expect(androidNeeds.size).toBe(1); // Android needs the updated entity

    expect(androidNeeds.has("todo-1")).toBe(true);

    const serverNeeds = computeVectorDiff(serverVector, clientVector);

    expect(serverNeeds.size).toBe(0); // Server doesn't need anything from Android
  });
});
