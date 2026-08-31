import { afterEach, expect, mock, test } from "bun:test";
import { ReconnectingEngineClient } from "@makekosmos/ark";

const firstLock = { http_port: 4317, auth_token: "a".repeat(64) };
const secondLock = { http_port: 4318, auth_token: "b".repeat(64) };
let discoveries = [firstLock, secondLock];
const originalFetch = globalThis.fetch;

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
  ensureEngineRunning: async () => ({ kind: "connected", lock: discoveries.shift()! }),
  ReconnectingEngineClient,
}));

const { EngineClient } = await import("./host-api");

afterEach(() => {
  discoveries = [firstLock, secondLock];
  globalThis.fetch = originalFetch;
});

test("Host retries only its idempotent lifecycle settings request after 401", async () => {
  let calls = 0;
  const fetchMock = mock(async (_input: string) => {
    calls += 1;
    if (calls === 1) return new Response("", { status: 401 });
    return new Response(
      JSON.stringify({ ok: true, data: { desktop_host: { warm_timeout_seconds: 300 } } }),
    );
  });
  // SAFETY: Bun's mock function has the same call signature as global fetch in this test.
  globalThis.fetch = fetchMock as typeof fetch;

  const client = new EngineClient("C:\\Kosmos-test");
  await expect(client.getWarmTimeout()).resolves.toEqual({ ok: true, data: 300 });
  expect(fetchMock.mock.calls.map((call: unknown[]) => call[0])).toEqual([
    "http://127.0.0.1:4317/v1/rpc",
    "http://127.0.0.1:4318/v1/rpc",
  ]);
});

test("Host does not replay launch after 401 and uses the replacement for the following request", async () => {
  let calls = 0;
  const fetchMock = mock(async (_input: string) => {
    calls += 1;
    if (calls === 1) return new Response("", { status: 401 });
    return new Response(
      JSON.stringify({ ok: true, data: { desktop_host: { warm_timeout_seconds: 0 } } }),
    );
  });
  // SAFETY: Bun's mock function has the same call signature as global fetch in this test.
  globalThis.fetch = fetchMock as typeof fetch;

  const client = new EngineClient("C:\\Kosmos-test");
  await expect(client.launchApp("demo")).resolves.toMatchObject({ ok: false });
  await expect(client.getWarmTimeout()).resolves.toEqual({ ok: true, data: 0 });
  expect(fetchMock.mock.calls.map((call: unknown[]) => call[0])).toEqual([
    "http://127.0.0.1:4317/v1/apps/launch",
    "http://127.0.0.1:4318/v1/rpc",
  ]);
});

test("Host does not replay revoke after a closed transport and uses the replacement for the following request", async () => {
  let calls = 0;
  const fetchMock = mock(async (_input: string) => {
    calls += 1;
    if (calls === 1) throw new Error("transport closed");
    return new Response(
      JSON.stringify({ ok: true, data: { desktop_host: { warm_timeout_seconds: 300 } } }),
    );
  });
  // SAFETY: Bun's mock function has the same call signature as global fetch in this test.
  globalThis.fetch = fetchMock as typeof fetch;

  const client = new EngineClient("C:\\Kosmos-test");
  await expect(client.revokeApp("123e4567-e89b-12d3-a456-426614174000")).resolves.toMatchObject({
    ok: false,
  });
  await expect(client.getWarmTimeout()).resolves.toEqual({ ok: true, data: 300 });
  expect(fetchMock.mock.calls.map((call: unknown[]) => call[0])).toEqual([
    "http://127.0.0.1:4317/v1/apps/launch/123e4567-e89b-12d3-a456-426614174000",
    "http://127.0.0.1:4318/v1/rpc",
  ]);
});

test("Host registers a directory grant without returning the selected path", async () => {
  const fetchMock = mock(async (input: string, init?: RequestInit) => {
    expect(input).toBe(
      "http://127.0.0.1:4317/v1/apps/launch/123e4567-e89b-12d3-a456-426614174000/grants/directory",
    );
    expect(init?.method).toBe("POST");
    expect(init?.headers).toMatchObject({
      "X-Kosmos-Launch-Token": "token",
    });
    expect(JSON.parse(String(init?.body))).toEqual({ selected_root: "C:\\Kosmos\\Selected" });
    return new Response(
      JSON.stringify({
        ok: true,
        data: { persistentGrantId: "persistent-grant", label: "Selected" },
      }),
    );
  });
  // SAFETY: Bun's mock function has the same call signature as global fetch in this test.
  globalThis.fetch = fetchMock as typeof fetch;

  const client = new EngineClient("C:\\Kosmos-test");
  await expect(
    client.registerDirectoryGrant(
      "123e4567-e89b-12d3-a456-426614174000",
      "token",
      "C:\\Kosmos\\Selected",
    ),
  ).resolves.toEqual({
    ok: true,
    data: { persistentGrantId: "persistent-grant", label: "Selected" },
  });
});

test("Host rejects a relative directory before contacting Engine", async () => {
  const fetchMock = mock(async () => new Response("unexpected", { status: 500 }));
  // SAFETY: Bun's mock function has the same call signature as global fetch in this test.
  globalThis.fetch = fetchMock as typeof fetch;

  const client = new EngineClient("C:\\Kosmos-test");
  await expect(
    client.registerDirectoryGrant(
      "123e4567-e89b-12d3-a456-426614174000",
      "token",
      "relative\\selected",
    ),
  ).resolves.toEqual({ ok: false, message: "Некорректный выбранный каталог." });
  expect(fetchMock).not.toHaveBeenCalled();
});
