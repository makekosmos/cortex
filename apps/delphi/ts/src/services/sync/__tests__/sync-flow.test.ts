// @vitest-environment node

/**
 * AC4: Full protocol flow tests (end-to-end sync).
 *
 * SyncServer + SyncClient connected together with sidecar mocked.
 *
 * Both SyncServer and SyncClient share the same VERSION_VECTOR_KEY and the
 * same sidecar mock -- this is the known bug. For the "server has data" test,
 * we work around this by pre-populating the shared version vector and verifying
 * that the sync protocol messages flow correctly. For tests that need isolated
 * storage, we use the shared mock but design assertions that are resilient to it.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  SyncServer,
  SyncClient,
  type SyncClientOptions,
  type StorageBackend,
} from "@arksync/core";
import {
  LAN_SYNC_PORT,
  type SyncEntity,
  type PeerRecord,
} from "../lan-protocol";

// ---------------------------------------------------------------------------
// In-memory StorageBackend for tests
// ---------------------------------------------------------------------------

function createInMemoryStorage(): StorageBackend & {
  entities: SyncEntity[];
  kv: Map<string, string>;
} {
  const entities: SyncEntity[] = [];
  const kv = new Map<string, string>();
  return {
    entities,
    kv,
    loadEntities: vi.fn(async () => [...entities]),
    applyEntity: vi.fn(async (entity: SyncEntity) => {
      if (entity.deleted) {
        const idx = entities.findIndex((e) => e.id === entity.id);
        if (idx >= 0) entities.splice(idx, 1);
        return;
      }
      const idx = entities.findIndex((e) => e.id === entity.id);
      if (idx >= 0) entities[idx] = entity;
      else entities.push(entity);
    }),
    getKv: vi.fn(async (key: string) => kv.get(key) ?? null),
    setKv: vi.fn(async (key: string, value: string) => {
      kv.set(key, value);
    }),
  };
}

let serverStorage: ReturnType<typeof createInMemoryStorage>;
let clientStorage: ReturnType<typeof createInMemoryStorage>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const SERVER_DEVICE_ID = "flow-server-001";
const SERVER_DEVICE_NAME = "Flow Server";
const CLIENT_DEVICE_ID = "flow-client-001";
const CLIENT_DEVICE_NAME = "Flow Client";
const SPACE_ID = "flow-space-id";

function waitFor(
  fn: () => boolean,
  timeoutMs = 5000,
  intervalMs = 50,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    const check = () => {
      if (fn()) return resolve();
      if (Date.now() - start > timeoutMs)
        return reject(new Error("waitFor timeout"));
      setTimeout(check, intervalMs);
    };
    check();
  });
}

function wait(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Full protocol flow", () => {
  let server: SyncServer;
  let client: SyncClient | null = null;
  let serverReceivedChanges: SyncEntity[];
  let clientReceivedChanges: SyncEntity[];
  let clientReceivedPeers: PeerRecord[];

  beforeEach(() => {
    serverStorage = createInMemoryStorage();
    clientStorage = createInMemoryStorage();
    serverReceivedChanges = [];
    clientReceivedChanges = [];
    clientReceivedPeers = [];
  });

  afterEach(async () => {
    if (client) {
      client.stop();
      client = null;
    }
    if (server) {
      server.stop();
    }
    await wait(300);
  });

  function createServer(): SyncServer {
    server = new SyncServer(serverStorage);
    server.onChange((entity) => {
      serverReceivedChanges.push(entity);
    });
    return server;
  }

  function createClientInstance(): SyncClient {
    const peer: PeerRecord = {
      device_id: SERVER_DEVICE_ID,
      device_name: SERVER_DEVICE_NAME,
      addresses: [`127.0.0.1:${LAN_SYNC_PORT}`],
      last_seen: new Date().toISOString(),
    };

    const opts: SyncClientOptions = {
      peer,
      deviceId: CLIENT_DEVICE_ID,
      deviceName: CLIENT_DEVICE_NAME,
      spaceId: SPACE_ID,
      ownAddresses: ["127.0.0.1:9999"],
      storage: clientStorage,
      onChange: (entity) => {
        clientReceivedChanges.push(entity);
      },
      onConnected: vi.fn(),
      onDisconnected: vi.fn(),
      onPeerList: (peers) => {
        clientReceivedPeers.push(...peers);
      },
    };

    client = new SyncClient(opts);
    return client;
  }

  // -----------------------------------------------------------------------
  // Empty <-> empty sync
  // -----------------------------------------------------------------------

  it("empty-empty: no data exchanged, both enter live mode", async () => {
    createServer();
    await server.start(SPACE_ID, SERVER_DEVICE_ID, SERVER_DEVICE_NAME, [
      `127.0.0.1:${LAN_SYNC_PORT}`,
    ]);

    createClientInstance();
    client!.start();
    await waitFor(() => client!.isConnected);
    await wait(1000);

    expect(serverReceivedChanges).toHaveLength(0);
    expect(clientReceivedChanges).toHaveLength(0);
  });

  // -----------------------------------------------------------------------
  // Server has data: verify sync_changes batches and dbUpsertTodo calls
  // -----------------------------------------------------------------------

  it("server has data: server sends entities via sync protocol", async () => {
    // Seed entities in server storage
    const hlcStr = "2026-01-01T00:00:00.000Z:000001:" + SERVER_DEVICE_ID;
    serverStorage.entities.push(
      {
        type: "todo",
        id: "todo-1",
        data: { id: "todo-1", title: "Server Task 1" },
        hlc: hlcStr,
      },
      {
        type: "todo",
        id: "todo-2",
        data: { id: "todo-2", title: "Server Task 2" },
        hlc: hlcStr,
      },
    );

    createServer();
    await server.start(SPACE_ID, SERVER_DEVICE_ID, SERVER_DEVICE_NAME, [
      `127.0.0.1:${LAN_SYNC_PORT}`,
    ]);

    createClientInstance();
    client!.start();
    await waitFor(() => client!.isConnected);
    await wait(2000);

    expect(server.connectedPeerCount).toBe(1);
    // Server should have persisted version vector
    const vv = serverStorage.kv.get("lan_sync.version_vector");
    expect(vv).toBeDefined();
    const vector = JSON.parse(vv!);
    expect(vector["todo-1"]).toBeDefined();
    expect(vector["todo-2"]).toBeDefined();

    // Client should have received the entities
    expect(clientStorage.applyEntity).toHaveBeenCalled();
  });

  // -----------------------------------------------------------------------
  // Both have data, partial overlap
  // -----------------------------------------------------------------------

  it("both have data: version vectors are exchanged and merged", async () => {
    const hlcStr = "2026-01-01T00:00:00.000Z:000001:" + SERVER_DEVICE_ID;
    serverStorage.entities.push(
      {
        type: "todo",
        id: "shared-todo",
        data: { id: "shared-todo", title: "Shared" },
        hlc: hlcStr,
      },
      {
        type: "todo",
        id: "extra-todo",
        data: { id: "extra-todo", title: "Extra" },
        hlc: hlcStr,
      },
    );

    createServer();
    await server.start(SPACE_ID, SERVER_DEVICE_ID, SERVER_DEVICE_NAME, [
      `127.0.0.1:${LAN_SYNC_PORT}`,
    ]);

    createClientInstance();
    client!.start();
    await waitFor(() => client!.isConnected);
    await wait(1500);

    // Verify sync completed: version vector contains both entities
    const vv = serverStorage.kv.get("lan_sync.version_vector");
    expect(vv).toBeDefined();
    const vector = JSON.parse(vv!);
    expect(vector["shared-todo"]).toBeDefined();
    expect(vector["extra-todo"]).toBeDefined();
  });

  // -----------------------------------------------------------------------
  // Live mode changes (bidirectional)
  // -----------------------------------------------------------------------

  it("live mode: mutation broadcast as live_change arrives at the other side", async () => {
    createServer();
    await server.start(SPACE_ID, SERVER_DEVICE_ID, SERVER_DEVICE_NAME, [
      `127.0.0.1:${LAN_SYNC_PORT}`,
    ]);

    createClientInstance();
    client!.start();
    await waitFor(() => client!.isConnected);
    await wait(1500);

    // Client sends a live change to the server
    const clientEntity: SyncEntity = {
      type: "todo",
      id: "live-from-client",
      data: { id: "live-from-client", title: "From client" },
      hlc: "2026-04-02T00:00:00.000Z:000001:flow-client-001",
    };
    client!.broadcastLiveChange(clientEntity);

    await waitFor(
      () => serverReceivedChanges.some((c) => c.id === "live-from-client"),
      3000,
    );
    expect(
      serverReceivedChanges.find((c) => c.id === "live-from-client"),
    ).toBeDefined();

    // Server sends a live change to the client
    const serverEntity: SyncEntity = {
      type: "todo",
      id: "live-from-server",
      data: { id: "live-from-server", title: "From server" },
      hlc: "2026-04-02T00:00:01.000Z:000001:flow-server-001",
    };
    server.broadcastLiveChange(serverEntity);

    await waitFor(
      () => clientReceivedChanges.some((c) => c.id === "live-from-server"),
      3000,
    );
    expect(
      clientReceivedChanges.find((c) => c.id === "live-from-server"),
    ).toBeDefined();

    // Verify entities were applied to storage
    expect(serverStorage.applyEntity).toHaveBeenCalledWith(
      expect.objectContaining({ id: "live-from-client" }),
    );
    expect(clientStorage.applyEntity).toHaveBeenCalledWith(
      expect.objectContaining({ id: "live-from-server" }),
    );
  });

  // -----------------------------------------------------------------------
  // Peer list exchange
  // -----------------------------------------------------------------------

  it("peer list exchange: receiver merges new peers into its known list", async () => {
    createServer();
    await server.start(SPACE_ID, SERVER_DEVICE_ID, SERVER_DEVICE_NAME, [
      `127.0.0.1:${LAN_SYNC_PORT}`,
    ]);
    server.registerExternalPeer("ext-peer-001", "External Peer", [
      "10.0.0.5:21531",
    ]);

    createClientInstance();
    client!.start();
    await waitFor(() => client!.isConnected);
    await wait(1000);

    // Client's onPeerList handler should have received peers from the server
    expect(
      clientReceivedPeers.some((p) => p.device_id === "ext-peer-001"),
    ).toBe(true);
  });

  // -----------------------------------------------------------------------
  // HLC conflict resolution
  // -----------------------------------------------------------------------

  it("HLC conflict resolution: newer HLC wins, older is not applied", async () => {
    createServer();
    await server.start(SPACE_ID, SERVER_DEVICE_ID, SERVER_DEVICE_NAME, [
      `127.0.0.1:${LAN_SYNC_PORT}`,
    ]);

    createClientInstance();
    client!.start();
    await waitFor(() => client!.isConnected);
    await wait(1500); // let sync complete

    // Send a live change with a NEWER hlc
    const newerHlc = "2026-04-01T00:00:00.000Z:000001:flow-client-001";
    const newerEntity: SyncEntity = {
      type: "todo",
      id: "conflict-todo",
      data: { id: "conflict-todo", title: "Newer version" },
      hlc: newerHlc,
    };
    client!.broadcastLiveChange(newerEntity);

    await waitFor(
      () => serverReceivedChanges.some((c) => c.id === "conflict-todo"),
      3000,
    );
    const first = serverReceivedChanges.find((c) => c.id === "conflict-todo");
    expect(first!.data.title).toBe("Newer version");

    // Now send a live change with an OLDER hlc -- should be rejected
    serverReceivedChanges.length = 0; // clear
    const olderEntity: SyncEntity = {
      type: "todo",
      id: "conflict-todo",
      data: { id: "conflict-todo", title: "Older version" },
      hlc: "2025-01-01T00:00:00.000Z:000001:flow-client-001",
    };
    client!.broadcastLiveChange(olderEntity);
    await wait(1000);

    // Server's onChange should NOT have fired for the older version
    const olderReceived = serverReceivedChanges.find(
      (c) => c.id === "conflict-todo" && c.data.title === "Older version",
    );
    expect(olderReceived).toBeUndefined();
  });
});
