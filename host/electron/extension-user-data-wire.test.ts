import { afterEach, describe, expect, mock, test } from "../test-support/node-test.mjs";
import { ReconnectingEngineClient } from "@makekosmos/ark";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { startFakeEngine } from "../test-support/fake-engine.mjs";

const TOKEN = "a".repeat(64);
const SECOND_TOKEN = "b".repeat(64);
const IO_ERROR = { ok: false, error: "io-error" };
let lock = { http_port: 0, ws_port: 0, auth_token: TOKEN };

class FakeArkClient {
  async start(): Promise<void> {}
  async stop(): Promise<void> {}
  async invokeOperation<T>(): Promise<T> {
    throw new Error("not used");
  }
  onArkEvent(): () => void {
    return () => {};
  }
}

mock.module("@makekosmos/ark", () => ({
  ArkClient: FakeArkClient,
  ensureEngineRunning: () => Promise.resolve({ kind: "connected", lock }),
  ReconnectingEngineClient,
}));

const { EngineClient } = await import("./host-api");
const { createEngineUserDataStores, USER_DATA_MAX_BYTES } =
  await import("./extension-user-data-ipc");

type FakeEngine = Awaited<ReturnType<typeof startFakeEngine>>;
const engines: FakeEngine[] = [];
const dirs: string[] = [];

afterEach(async () => {
  while (engines.length) await engines.pop()!.close();
  while (dirs.length) rmSync(dirs.pop()!, { recursive: true, force: true });
  lock = { http_port: 0, ws_port: 0, auth_token: TOKEN };
});

// Mirrors the wiring in main.ts: EngineClient primitives under
// createEngineUserDataStores, pinned to the Host userData directory.
async function harness(rootName = "") {
  const engine = await startFakeEngine(TOKEN);
  engines.push(engine);
  const base = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-wire-"));
  dirs.push(base);
  const userDataRoot = rootName ? path.join(base, rootName) : base;
  lock = { http_port: engine.port, ws_port: 0, auth_token: TOKEN };
  const client = new EngineClient("C:\\Kosmos-wire-test");
  const stores = createEngineUserDataStores(
    {
      openRoot: (root) => client.userDataOpenRoot(root),
      read: (rootId, appId, key) => client.userDataRead(rootId, appId, key),
      write: (rootId, appId, key, bytes) => client.userDataWrite(rootId, appId, key, bytes),
      stat: (rootId, appId, key) => client.userDataStat(rootId, appId, key),
      delete: (rootId, appId, key) => client.userDataDelete(rootId, appId, key),
    },
    userDataRoot,
  );
  return { engine, client, stores, userDataRoot };
}

const stored = (root: string, app: string, key: string) =>
  path.join(root, "extension-data", app, ...key.split("/"));
const kinds = (engine: FakeEngine, from = 0) =>
  engine.requests.slice(from).map((r) => `${r.operation}:${r.status}`);
const rootId = async (client: InstanceType<typeof EngineClient>, root: string) => {
  const open = await client.userDataOpenRoot(root);
  expect(open).toMatchObject({ ok: true });
  if (!open.ok) throw new Error("open_root rejected");
  return open.data.rootId;
};

describe("user data over the real Engine HTTP boundary", () => {
  test("round-trips binary payloads through raw PUT bodies and JSON operations", async () => {
    const { engine, stores, userDataRoot } = await harness();
    const store = stores.forApp("com.kosmos.agenda");

    expect(await store.read("missing.bin")).toEqual({ ok: false, error: "not-found" });
    expect(await store.write("attachments/task-1.bin", new Uint8Array([0, 1, 255]))).toEqual({
      ok: true,
      data: { sizeBytes: 3 },
    });
    const onDisk = stored(userDataRoot, "com.kosmos.agenda", "attachments/task-1.bin");
    expect(readFileSync(onDisk)).toEqual(Buffer.from([0, 1, 255]));
    expect(await store.stat("attachments/task-1.bin")).toEqual({
      ok: true,
      data: { sizeBytes: 3 },
    });
    expect(await store.read("attachments/task-1.bin")).toEqual({
      ok: true,
      data: new Uint8Array([0, 1, 255]),
    });
    expect(await store.delete("attachments/task-1.bin")).toEqual({ ok: true, data: true });
    expect(existsSync(onDisk)).toBe(false);

    expect(kinds(engine)).toEqual([
      "open_root:200",
      "read:404",
      "write:200",
      "stat:200",
      "read:200",
      "delete:200",
    ]);
    for (const request of engine.requests) {
      expect(request.headers.authorization).toBe(`Bearer ${TOKEN}`);
      expect(request.headers["x-kosmos-client-class"]).toBe("desktop-host");
      expect(request.headers["x-kosmos-api-version"]).toBe("1.0.0");
      expect(request.headers["x-kosmos-client-pid"]).toBe(String(process.pid));
    }
    const put = engine.requests.find((request) => request.method === "PUT")!;
    expect(put.headers["x-kosmos-user-data-app"]).toBe("com.kosmos.agenda");
    expect(put.headers["x-kosmos-user-data-key"]).toBe("attachments/task-1.bin");
    expect(put.headers["x-kosmos-user-data-root"]).toMatch(/^[0-9a-f-]{36}$/);
    expect(put.bodyBytes).toBe(3);
  });

  test("re-authenticates and re-pins the root after an Engine restart", async () => {
    const { engine, stores } = await harness();
    const store = stores.forApp("com.kosmos.agenda");
    await store.write("a.bin", new Uint8Array([7]));
    const before = engine.requests.length;

    // A restart loses registered roots and rotates the bearer token; the file
    // itself survives on disk.
    engine.restart();
    engine.expectedToken = SECOND_TOKEN;
    lock = { http_port: engine.port, ws_port: 0, auth_token: SECOND_TOKEN };

    expect(await store.read("a.bin")).toEqual({ ok: true, data: new Uint8Array([7]) });
    expect(kinds(engine, before)).toEqual(["read:401", "read:404", "open_root:200", "read:200"]);
    expect(engine.requests[before + 1].headers.authorization).toBe(`Bearer ${SECOND_TOKEN}`);
  });

  test("preserves typed Engine error codes across the wire", async () => {
    const { engine, client, stores, userDataRoot } = await harness();
    const pinned = await rootId(client, userDataRoot);

    expect(await client.userDataRead(pinned, "app", "../escape")).toEqual({
      ok: false,
      error: "invalid-key",
    });
    expect(await client.userDataRead(pinned, "app", "missing.bin")).toEqual({
      ok: false,
      error: "not-found",
    });
    expect(await client.userDataRead("gone-root", "app", "a.bin")).toEqual({
      ok: false,
      error: "unknown-root",
    });

    const requestsBefore = engine.requests.length;
    expect(await client.userDataOpenRoot("relative/root")).toEqual({
      ok: false,
      error: "invalid-request",
    });
    expect(await stores.forApp("app").read("../escape")).toEqual({
      ok: false,
      error: "invalid-key",
    });
    expect(engine.requests.length).toBe(requestsBefore);
  });

  test("fails closed on malformed or oversized Engine responses", async () => {
    const { engine, client, stores, userDataRoot } = await harness();
    for (const payload of [{ ok: true, data: { root_id: "../escape" } }, { ok: true }]) {
      engine.hijack = (operation) =>
        operation === "open_root" ? { status: 200, payload } : undefined;
      expect(await client.userDataOpenRoot(userDataRoot)).toEqual(IO_ERROR);
    }

    engine.hijack = null;
    const pinned = await rootId(client, userDataRoot);
    const call = (operation: string) =>
      operation === "read"
        ? client.userDataRead(pinned, "app", "a.bin")
        : operation === "stat"
          ? client.userDataStat(pinned, "app", "a.bin")
          : client.userDataDelete(pinned, "app", "a.bin");
    for (const [operation, payload] of [
      ["read", "not json"],
      ["read", { ok: true, data: { bytes: 123 } }],
      ["read", { ok: true, data: { bytes: "A".repeat(37_000_000) } }],
      ["stat", { ok: true, data: { size_bytes: -1 } }],
      ["delete", { ok: true, data: { deleted: false } }],
    ] as const) {
      engine.hijack = (op) => (op === operation ? { status: 200, payload } : undefined);
      expect(await call(operation)).toEqual(IO_ERROR);
    }

    // A write reporting a size different from the sent payload is caught by
    // the store-level contract check.
    engine.hijack = (operation) =>
      operation === "write"
        ? { status: 200, payload: { ok: true, data: { size_bytes: 999 } } }
        : undefined;
    expect(await stores.forApp("app").write("a.bin", new Uint8Array([1]))).toEqual(IO_ERROR);
  });

  test("enforces the 25 MiB contract on the real channel", async () => {
    const { engine, client, stores, userDataRoot } = await harness();
    const store = stores.forApp("app");
    const limit = new Uint8Array(USER_DATA_MAX_BYTES);
    limit[0] = 1;
    limit[USER_DATA_MAX_BYTES - 1] = 2;
    expect(await store.write("limit.bin", limit)).toEqual({
      ok: true,
      data: { sizeBytes: USER_DATA_MAX_BYTES },
    });
    expect(await store.read("limit.bin")).toEqual({ ok: true, data: limit });

    const requestsBefore = engine.requests.length;
    expect(await store.write("too-big.bin", new Uint8Array(USER_DATA_MAX_BYTES + 1))).toEqual({
      ok: false,
      error: "too-large",
    });
    expect(engine.requests.length).toBe(requestsBefore);

    const pinned = await rootId(client, userDataRoot);
    expect(
      await client.userDataWrite(pinned, "app", "big.bin", new Uint8Array(USER_DATA_MAX_BYTES + 1)),
    ).toEqual({ ok: false, error: "too-large" });
    expect(existsSync(stored(userDataRoot, "app", "big.bin"))).toBe(false);
  });

  test("creates a missing userData root before pinning", async () => {
    const { engine, stores, userDataRoot } = await harness("missing/userData");
    expect(existsSync(userDataRoot)).toBe(false);
    expect(await stores.forApp("app").write("a.bin", new Uint8Array([1]))).toEqual({
      ok: true,
      data: { sizeBytes: 1 },
    });
    expect(existsSync(userDataRoot)).toBe(true);
    expect(
      engine.requests
        .filter((request) => request.operation === "open_root")
        .map((request) => request.status),
    ).toEqual([404, 200]);
  });

  test("reads files written by the pre-KOS-49 pathname store", async () => {
    const { stores, userDataRoot } = await harness();
    const legacy = stored(userDataRoot, "com.kosmos.agenda", "attachments/legacy.bin");
    mkdirSync(path.dirname(legacy), { recursive: true });
    writeFileSync(legacy, Buffer.from([9, 8, 7]));
    const store = stores.forApp("com.kosmos.agenda");
    expect(await store.read("attachments/legacy.bin")).toEqual({
      ok: true,
      data: new Uint8Array([9, 8, 7]),
    });
    expect(await store.stat("attachments/legacy.bin")).toEqual({
      ok: true,
      data: { sizeBytes: 3 },
    });
  });

  test("replays idempotent writes but not deletes after a rejected request", async () => {
    const { engine, stores } = await harness();
    const store = stores.forApp("app");
    await store.write("a.bin", new Uint8Array([1]));

    // A delete is not idempotent: one attempt, then the error surfaces.
    engine.expectedToken = SECOND_TOKEN;
    expect(await store.delete("a.bin")).toEqual(IO_ERROR);
    const deletes = engine.requests.filter((request) => request.operation === "delete");
    expect(deletes).toHaveLength(1);
    expect(deletes[0].status).toBe(401);

    // A PUT carries the full replacement bytes, so one replay is safe; the
    // second rejection ends the call.
    expect(await store.write("b.bin", new Uint8Array([2]))).toEqual(IO_ERROR);
    const puts = engine.requests.filter((request) => request.method === "PUT");
    expect(puts.slice(-2).map((request) => request.status)).toEqual([401, 401]);

    expect(await store.read("a.bin")).toEqual(IO_ERROR);
    const reads = engine.requests.filter((request) => request.operation === "read");
    expect(reads.slice(-2).map((request) => request.status)).toEqual([401, 401]);
  });
});
