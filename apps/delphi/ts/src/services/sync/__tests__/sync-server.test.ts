// @vitest-environment node

/**
 * AC2: SyncServer integration tests.
 *
 * Uses real WebSocket connections (ws library) against a real SyncServer instance
 * with the sidecar module fully mocked.
 */

import { describe, it, expect, beforeAll, afterAll, vi } from "vitest";
import WebSocket from "ws";

import { SyncServer } from "@arksync/core";
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
  LAN_SYNC_PORT,
  type HelloMessage,
  type LanSyncMessage,
  type LiveChangeMessage,
  type SyncEntity,
  serializeMessage,
  deserializeMessage,
} from "../lan-protocol";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const SERVER_DEVICE_ID = "server-device-001";
const SERVER_DEVICE_NAME = "Test Server";
const SPACE_ID = "test-space-id";

/** Connect a raw ws client and perform hello handshake; returns the ws + server's hello response. */
function connectAndHello(
  port: number,
  opts?: { deviceId?: string; deviceName?: string; protocolVersion?: number },
): Promise<{ ws: WebSocket; hello: HelloMessage }> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}`);
    ws.on("open", () => {
      const helloMsg: HelloMessage = {
        type: "hello",
        protocol_version: opts?.protocolVersion ?? PROTOCOL_VERSION,
        device_id: opts?.deviceId ?? `client-${Date.now()}`,
        device_name: opts?.deviceName ?? "Test Client",
        space_id: SPACE_ID,
        addresses: ["127.0.0.1:9999"],
      };
      ws.send(serializeMessage(helloMsg));
    });

    ws.on("message", (raw) => {
      const msg = deserializeMessage(raw.toString());
      if (msg && msg.type === "hello") {
        resolve({ ws, hello: msg as HelloMessage });
      }
    });

    ws.on("error", reject);

    // Timeout
    setTimeout(() => reject(new Error("connectAndHello timeout")), 5000);
  });
}

/** Wait for next message of a given type. */
function waitForMessage(
  ws: WebSocket,
  type: string,
  timeoutMs = 3000,
): Promise<LanSyncMessage> {
  return new Promise((resolve, reject) => {
    const handler = (raw: WebSocket.Data) => {
      const msg = deserializeMessage(raw.toString());
      if (msg && msg.type === type) {
        ws.off("message", handler);
        resolve(msg);
      }
    };
    ws.on("message", handler);
    setTimeout(() => {
      ws.off("message", handler);
      reject(new Error(`Timeout waiting for message type: ${type}`));
    }, timeoutMs);
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("SyncServer integration", () => {
  let server: SyncServer;
  const port = LAN_SYNC_PORT; // Use the default port

  beforeAll(async () => {
    server = new SyncServer(createMockStorage());
    await server.start(SPACE_ID, SERVER_DEVICE_ID, SERVER_DEVICE_NAME, [
      "127.0.0.1:21531",
    ]);
  });

  afterAll(() => {
    server.stop();
  });

  // -----------------------------------------------------------------------
  // Server starts and accepts connections
  // -----------------------------------------------------------------------

  it("starts and accepts WS connections", async () => {
    const { ws, hello } = await connectAndHello(port);
    expect(hello.type).toBe("hello");
    ws.close();
    await new Promise((r) => setTimeout(r, 100));
  });

  // -----------------------------------------------------------------------
  // Hello handshake
  // -----------------------------------------------------------------------

  it("responds with hello containing device_id, device_name, protocol_version, addresses", async () => {
    const { ws, hello } = await connectAndHello(port);
    expect(hello.protocol_version).toBe(PROTOCOL_VERSION);
    expect(hello.device_id).toBe(SERVER_DEVICE_ID);
    expect(hello.device_name).toBe(SERVER_DEVICE_NAME);
    expect(hello.addresses).toEqual(["127.0.0.1:21531"]);
    ws.close();
    await new Promise((r) => setTimeout(r, 100));
  });

  // -----------------------------------------------------------------------
  // Protocol version mismatch
  // -----------------------------------------------------------------------

  it("closes connection on protocol version mismatch (close code 1002)", async () => {
    const closePromise = new Promise<{ code: number }>((resolve) => {
      const ws = new WebSocket(`ws://127.0.0.1:${port}`);
      ws.on("open", () => {
        ws.send(
          serializeMessage({
            type: "hello",
            protocol_version: 9999,
            device_id: "bad-client",
            device_name: "Bad Client",
            space_id: SPACE_ID,
            addresses: [],
          }),
        );
      });
      ws.on("close", (code) => {
        resolve({ code });
      });
    });
    const { code } = await closePromise;
    expect(code).toBe(1002);
  });

  // -----------------------------------------------------------------------
  // getConnectedPeerNames dedup
  // -----------------------------------------------------------------------

  it("getConnectedPeerNames returns deduplicated names", async () => {
    const deviceId = "dedup-device-001";
    const { ws: ws1 } = await connectAndHello(port, {
      deviceId,
      deviceName: "My Phone",
    });
    const { ws: ws2 } = await connectAndHello(port, {
      deviceId,
      deviceName: "My Phone",
    });

    // Allow server to process
    await new Promise((r) => setTimeout(r, 200));

    const names = server.getConnectedPeerNames();
    // Even with 2 connections from the same device_id, we should see 1 name
    expect(names.filter((n) => n === "My Phone").length).toBe(1);

    ws1.close();
    ws2.close();
    await new Promise((r) => setTimeout(r, 100));
  });

  // -----------------------------------------------------------------------
  // getConnectedPeerEntries
  // -----------------------------------------------------------------------

  it("getConnectedPeerEntries returns deduplicated entries", async () => {
    const deviceId = "entry-device-001";
    const { ws: ws1 } = await connectAndHello(port, {
      deviceId,
      deviceName: "Tablet",
    });
    const { ws: ws2 } = await connectAndHello(port, {
      deviceId,
      deviceName: "Tablet",
    });

    await new Promise((r) => setTimeout(r, 200));

    const entries = server.getConnectedPeerEntries();
    const matching = entries.filter((e) => e.deviceId === deviceId);
    expect(matching.length).toBe(1);
    expect(matching[0].deviceName).toBe("Tablet");

    ws1.close();
    ws2.close();
    await new Promise((r) => setTimeout(r, 100));
  });

  // -----------------------------------------------------------------------
  // connectedPeerCount
  // -----------------------------------------------------------------------

  it("connectedPeerCount reflects authenticated peer count", async () => {
    const before = server.connectedPeerCount;
    const { ws } = await connectAndHello(port, { deviceId: "count-device" });
    await new Promise((r) => setTimeout(r, 100));
    expect(server.connectedPeerCount).toBeGreaterThan(before);
    ws.close();
    await new Promise((r) => setTimeout(r, 200));
  });

  // -----------------------------------------------------------------------
  // broadcastLiveChange
  // -----------------------------------------------------------------------

  it("broadcastLiveChange sends live_change to authenticated peers except excluded", async () => {
    // Connect two clients that complete the full sync protocol.
    // After hello, the server sends version_vector. We respond with version_vector,
    // then respond to sync_changes with sync_ack, and send our own sync_changes.
    function connectAndCompleteSyncHandshake(
      deviceId: string,
      deviceName: string,
    ): Promise<WebSocket> {
      return new Promise((resolve, reject) => {
        const ws = new WebSocket(`ws://127.0.0.1:${port}`);
        let helloDone = false;
        let sentVv = false;
        let sentSyncChanges = false;

        ws.on("open", () => {
          const helloMsg: HelloMessage = {
            type: "hello",
            protocol_version: PROTOCOL_VERSION,
            device_id: deviceId,
            device_name: deviceName,
            space_id: "test-space-id",
            addresses: ["127.0.0.1:9999"],
          };
          ws.send(serializeMessage(helloMsg));
        });

        ws.on("message", (raw) => {
          const msg = deserializeMessage(raw.toString());
          if (!msg) return;

          if (msg.type === "hello") {
            helloDone = true;
          }

          if (msg.type === "version_vector" && helloDone && !sentVv) {
            sentVv = true;
            ws.send(serializeMessage({ type: "version_vector", vector: {} }));
          }

          if (msg.type === "sync_changes" && helloDone) {
            ws.send(
              serializeMessage({
                type: "sync_ack",
                batch_id: (msg as any).batch_id,
                accepted: (msg as any).entities.length,
              }),
            );

            if ((msg as any).is_last && !sentSyncChanges) {
              sentSyncChanges = true;
              ws.send(
                serializeMessage({
                  type: "sync_changes",
                  batch_id: `batch-${deviceId}`,
                  entities: [],
                  is_last: true,
                }),
              );
              // Handshake complete -- give server a moment to mark syncComplete
              setTimeout(() => resolve(ws), 300);
            }
          }
        });

        ws.on("error", reject);
        setTimeout(() => reject(new Error("handshake timeout")), 8000);
      });
    }

    const ws1 = await connectAndCompleteSyncHandshake("peer-A", "Peer A");
    const ws2 = await connectAndCompleteSyncHandshake("peer-B", "Peer B");

    // Listen for live_change on peer-B's socket
    const msgPromise = waitForMessage(ws2, "live_change", 3000);

    const entity: SyncEntity = {
      type: "todo",
      id: "test-todo-1",
      data: { id: "test-todo-1", title: "Test" },
      hlc: "2026-04-01T00:00:00.000Z:000001:external",
    };

    // Broadcast excluding peer-A -> peer-B should receive it
    server.broadcastLiveChange(entity, "peer-A");

    const received = await msgPromise;
    expect(received.type).toBe("live_change");
    expect((received as LiveChangeMessage).entity.id).toBe("test-todo-1");

    ws1.close();
    ws2.close();
    await new Promise((r) => setTimeout(r, 100));
  }, 20000);

  // -----------------------------------------------------------------------
  // registerExternalPeer
  // -----------------------------------------------------------------------

  it("registerExternalPeer adds peer to known records", () => {
    server.registerExternalPeer("ext-device-001", "External Peer", [
      "10.0.0.5:21531",
    ]);
    const peers = server.getKnownPeers();
    const found = peers.find((p) => p.device_id === "ext-device-001");
    expect(found).toBeDefined();
    expect(found!.device_name).toBe("External Peer");
    expect(found!.addresses).toContain("10.0.0.5:21531");
  });

  // -----------------------------------------------------------------------
  // isConnectedTo
  // -----------------------------------------------------------------------

  it("isConnectedTo returns true for authenticated peers, false otherwise", async () => {
    expect(server.isConnectedTo("connected-test-device")).toBe(false);
    const { ws } = await connectAndHello(port, {
      deviceId: "connected-test-device",
    });
    await new Promise((r) => setTimeout(r, 100));
    expect(server.isConnectedTo("connected-test-device")).toBe(true);
    ws.close();
    await new Promise((r) => setTimeout(r, 200));
    expect(server.isConnectedTo("connected-test-device")).toBe(false);
  });

  // -----------------------------------------------------------------------
  // stop
  // -----------------------------------------------------------------------

  it("stop closes all connections and clears state", async () => {
    // Connect a client first
    const { ws } = await connectAndHello(port, {
      deviceId: "stop-test-client",
    });
    await new Promise((r) => setTimeout(r, 100));
    expect(server.connectedPeerCount).toBeGreaterThan(0);

    const closePromise = new Promise<void>((resolve) => {
      ws.on("close", () => resolve());
    });

    server.stop();
    await closePromise;
    expect(server.connectedPeerCount).toBe(0);

    // Restart the server so afterAll's stop() is safe
    await server.start(SPACE_ID, SERVER_DEVICE_ID, SERVER_DEVICE_NAME, [
      "127.0.0.1:21531",
    ]);
  });
});
