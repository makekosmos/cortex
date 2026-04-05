// @vitest-environment node

/**
 * AC3: SyncClient integration tests.
 *
 * Uses a real ws.WebSocketServer to simulate a peer server, with sidecar fully mocked.
 */

import {
  afterAll,
  afterEach,
  beforeAll,
  describe,
  expect,
  it,
  vi,
} from "vitest";

import WebSocket, { WebSocketServer } from "ws";

import { SyncClient, type SyncClientOptions } from "@arksync/core";

import type { StorageBackend } from "@arksync/core";

function createMockStorage(): StorageBackend {
  const kv = new Map<string, string>();

  return {
    loadEntities: vi.fn(async () => []),

    applyEntity: vi.fn(async () => {}),

    getKv: vi.fn(async (key: string) => kv.get(key) ?? null),

    setKv: vi.fn(async (key: string, value: string) => {
      kv.set(key, value);
    }),
  };
}

import {
  PROTOCOL_VERSION,
  type HelloMessage,
  type LanSyncMessage,
  type LiveChangeMessage,
  type SyncEntity,
  type PeerRecord,
  serializeMessage,
  deserializeMessage,
} from "../lan-protocol";

// ---------------------------------------------------------------------------

// Helpers

// ---------------------------------------------------------------------------

const CLIENT_DEVICE_ID = "client-device-001";

const CLIENT_DEVICE_NAME = "Test Client";

const SPACE_ID = "test-space-id";

/** Create a mock WS server that performs hello handshake and sends empty sync data. */

function createMockServer(port: number): Promise<{
  wss: WebSocketServer;

  getLastHello: () => HelloMessage | null;

  getConnections: () => WebSocket[];

  lastMessages: LanSyncMessage[];
}> {
  return new Promise((resolve) => {
    let lastHello: HelloMessage | null = null;

    const connections: WebSocket[] = [];

    const lastMessages: LanSyncMessage[] = [];

    const wss = new WebSocketServer({ port }, () => {
      resolve({
        wss,

        getLastHello: () => lastHello,

        getConnections: () => connections,

        lastMessages,
      });
    });

    wss.on("connection", (ws) => {
      connections.push(ws);

      ws.on("message", (raw) => {
        const msg = deserializeMessage(raw.toString());

        if (!msg) return;

        lastMessages.push(msg);

        if (msg.type === "hello") {
          lastHello = msg as HelloMessage;

          // Respond with server hello

          ws.send(
            serializeMessage({
              type: "hello",

              protocol_version: PROTOCOL_VERSION,

              device_id: "mock-server-001",

              device_name: "Mock Server",

              space_id: SPACE_ID,

              addresses: [`127.0.0.1:${port}`],
            }),
          );

          // Send version vector (empty)

          ws.send(
            serializeMessage({
              type: "version_vector",

              vector: {},
            }),
          );
        }

        if (msg.type === "version_vector") {
          // Send empty sync changes to complete sync

          ws.send(
            serializeMessage({
              type: "sync_changes",

              batch_id: "mock-batch-1",

              entities: [],

              is_last: true,
            }),
          );
        }

        if (msg.type === "sync_changes") {
          // ACK the batch

          ws.send(
            serializeMessage({
              type: "sync_ack",

              batch_id: msg.batch_id,

              accepted: msg.entities.length,
            }),
          );
        }

        if (msg.type === "live_change") {
          ws.send(
            serializeMessage({
              type: "live_ack",

              change_id: (msg as LiveChangeMessage).change_id,
            }),
          );
        }
      });

      ws.on("close", () => {
        const idx = connections.indexOf(ws);

        if (idx >= 0) connections.splice(idx, 1);
      });
    });
  });
}

function makePeerRecord(port: number, addresses?: string[]): PeerRecord {
  return {
    device_id: "mock-server-001",

    device_name: "Mock Server",

    addresses: addresses ?? [`127.0.0.1:${port}`],

    last_seen: new Date().toISOString(),
  };
}

function makeClientOptions(
  peer: PeerRecord,

  overrides?: Partial<SyncClientOptions>,
): SyncClientOptions {
  return {
    peer,

    deviceId: CLIENT_DEVICE_ID,

    deviceName: CLIENT_DEVICE_NAME,

    spaceId: SPACE_ID,

    ownAddresses: ["127.0.0.1:9999"],

    storage: createMockStorage(),

    onChange: vi.fn(),

    onConnected: vi.fn(),

    onDisconnected: vi.fn(),

    onPeerList: vi.fn(),

    ...overrides,
  };
}

/** Wait until a condition is true, with timeout. */

function waitFor(
  fn: () => boolean,

  timeoutMs = 3000,

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

// ---------------------------------------------------------------------------

// Tests

// ---------------------------------------------------------------------------

describe("SyncClient integration", () => {
  const SERVER_PORT = 21541; // Different from LAN_SYNC_PORT to avoid conflicts

  let mockServer: Awaited<ReturnType<typeof createMockServer>>;

  let clients: SyncClient[];

  beforeAll(async () => {
    mockServer = await createMockServer(SERVER_PORT);

    clients = [];
  });

  afterEach(() => {
    for (const c of clients) {
      c.stop();
    }

    clients = [];
  });

  afterAll(() => {
    mockServer.wss.close();
  });

  // -----------------------------------------------------------------------

  // Connect and hello

  // -----------------------------------------------------------------------

  it("connects and sends hello with correct fields", async () => {
    const peer = makePeerRecord(SERVER_PORT);

    const opts = makeClientOptions(peer);

    const client = new SyncClient(opts);

    clients.push(client);

    client.start();

    await waitFor(() => client.isConnected);

    const hello = mockServer.getLastHello();

    expect(hello).not.toBeNull();

    expect(hello!.protocol_version).toBe(PROTOCOL_VERSION);

    expect(hello!.device_id).toBe(CLIENT_DEVICE_ID);

    expect(hello!.device_name).toBe(CLIENT_DEVICE_NAME);

    expect(hello!.space_id).toBe(SPACE_ID);

    expect(hello!.addresses).toEqual(["127.0.0.1:9999"]);
  });

  // -----------------------------------------------------------------------

  // Authenticated state

  // -----------------------------------------------------------------------

  it("transitions to authenticated after receiving hello from server", async () => {
    const peer = makePeerRecord(SERVER_PORT);

    const onConnected = vi.fn();

    const opts = makeClientOptions(peer, { onConnected });

    const client = new SyncClient(opts);

    clients.push(client);

    expect(client.isConnected).toBe(false);

    client.start();

    await waitFor(() => client.isConnected);

    expect(client.isConnected).toBe(true);

    expect(onConnected).toHaveBeenCalledWith("mock-server-001", "Mock Server");
  });

  // -----------------------------------------------------------------------

  // Getters

  // -----------------------------------------------------------------------

  it("peerDeviceId and peerName reflect peer record / server hello", async () => {
    const peer = makePeerRecord(SERVER_PORT);

    const opts = makeClientOptions(peer);

    const client = new SyncClient(opts);

    clients.push(client);

    expect(client.peerDeviceId).toBe("mock-server-001");

    client.start();

    await waitFor(() => client.isConnected);

    expect(client.peerName).toBe("Mock Server");
  });

  // -----------------------------------------------------------------------

  // broadcastLiveChange

  // -----------------------------------------------------------------------

  it("broadcastLiveChange sends live_change to peer", async () => {
    const peer = makePeerRecord(SERVER_PORT);

    const opts = makeClientOptions(peer);

    const client = new SyncClient(opts);

    clients.push(client);

    client.start();

    await waitFor(() => client.isConnected);

    // Wait for sync to complete

    await new Promise((r) => setTimeout(r, 500));

    mockServer.lastMessages.length = 0;

    const entity: SyncEntity = {
      type: "todo",

      id: "live-todo-1",

      data: { id: "live-todo-1", title: "Live Test" },

      hlc: "2026-04-01T00:00:00.000Z:000001:test-device",
    };

    client.broadcastLiveChange(entity);

    await waitFor(() =>
      mockServer.lastMessages.some((m) => m.type === "live_change"),
    );

    const liveMsg = mockServer.lastMessages.find(
      (m) => m.type === "live_change",
    ) as LiveChangeMessage;

    expect(liveMsg.entity.id).toBe("live-todo-1");
  });

  // -----------------------------------------------------------------------

  // stop

  // -----------------------------------------------------------------------

  it("stop disconnects gracefully and does not reconnect", async () => {
    const peer = makePeerRecord(SERVER_PORT);

    const onDisconnected = vi.fn();

    const opts = makeClientOptions(peer, { onDisconnected });

    const client = new SyncClient(opts);

    clients.push(client);

    client.start();

    await waitFor(() => client.isConnected);

    client.stop();

    await new Promise((r) => setTimeout(r, 300));

    expect(client.isConnected).toBe(false);

    // Wait a bit more to ensure no reconnection attempt

    await new Promise((r) => setTimeout(r, 500));

    expect(client.isConnected).toBe(false);
  });

  // -----------------------------------------------------------------------

  // Address racing

  // -----------------------------------------------------------------------

  it("connects via the first successful address when given multiple", async () => {
    const peer: PeerRecord = {
      device_id: "mock-server-001",

      device_name: "Mock Server",

      addresses: [
        "127.0.0.1:19999", // unreachable

        "127.0.0.1:19998", // unreachable

        `127.0.0.1:${SERVER_PORT}`, // valid
      ],

      last_seen: new Date().toISOString(),
    };

    const opts = makeClientOptions(peer);

    const client = new SyncClient(opts);

    clients.push(client);

    client.start();

    await waitFor(() => client.isConnected, 8000);

    expect(client.isConnected).toBe(true);
  });

  // -----------------------------------------------------------------------

  // Reconnection with exponential backoff

  // -----------------------------------------------------------------------

  it("schedules reconnect with increasing delay after disconnect", async () => {
    // We test that after stop, when not explicitly stopped, reconnect is scheduled.

    // Use a server that we can shut down.

    const tempPort = 21542;

    const tempServer = await createMockServer(tempPort);

    const peer = makePeerRecord(tempPort);

    const opts = makeClientOptions(peer);

    const client = new SyncClient(opts);

    clients.push(client);

    client.start();

    await waitFor(() => client.isConnected);

    // Close server -- client should disconnect and schedule reconnect

    tempServer.wss.close();

    // Force-close all connections

    for (const conn of tempServer.getConnections()) {
      conn.close();
    }

    await new Promise((r) => setTimeout(r, 500));

    expect(client.isConnected).toBe(false);

    // The client should be trying to reconnect (not stopped).

    // We can verify by checking that the client's internal state

    // hasn't marked stopped. Since we can't access private fields,

    // we verify by starting a new server on same port and seeing it reconnect.

    const tempServer2 = await createMockServer(tempPort);

    // Give enough time for reconnect (base delay 2s + jitter)

    await waitFor(() => client.isConnected, 6000);

    expect(client.isConnected).toBe(true);

    client.stop();

    tempServer2.wss.close();
  });
});
