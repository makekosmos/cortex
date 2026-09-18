import { afterEach, expect, mock, test } from "../test-support/node-test.mjs";
import { ReconnectingEngineClient } from "@makekosmos/ark";

// The shared node:test adapter relies on `mock.fn`/`mock.module`, which bun's
// node:test compatibility shim does not implement (under bun, `mock` is a bare
// callable). The check-plan gate runs this file through `bun test`, so under
// bun both the module mock and function mocks delegate to bun:test's own
// implementation; under node --test the adapter path is unchanged.
// SAFETY: ProcessVersions has no `bun` field in the installed @types/node; the
// record cast only probes for the runtime marker.
const runningUnderBun = (process.versions as Record<string, string | undefined>).bun !== undefined;

// node:test records calls as `{ arguments: [...] }`; bun:test records raw
// argument arrays.
type RecordedCall = { arguments: string[] } | string[];
type AnyMockFn = { mock: { calls: RecordedCall[] } };
type FetchImpl = (input: string, init?: RequestInit) => Promise<Response>;
type BunMock = {
  (implementation: FetchImpl): FetchImpl & AnyMockFn;
  module(specifier: string, factory: () => object): void;
};

const bunTestSpecifier = "bun:test";
const bunMock: BunMock | null = runningUnderBun
  ? // SAFETY: import() runs only under bun, where it resolves to bun:test; the
    // asserted shape is bun's own documented mock surface.
    ((await import(bunTestSpecifier)) as { mock: BunMock }).mock
  : null;

const mockFn = (implementation: FetchImpl): FetchImpl & AnyMockFn => {
  const created = bunMock ? bunMock(implementation) : mock(implementation);
  // SAFETY: both runners wrap the implementation in a mock with the same call
  // signature; only the recorded-call shape differs, normalized below.
  return created as FetchImpl & AnyMockFn;
};

const firstCallArgs = (fn: AnyMockFn): string[] =>
  fn.mock.calls.map((call) => (Array.isArray(call) ? call[0] : call.arguments[0]));

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

const fakeArkModule = () => ({
  ArkClient: FakeArkClient,
  ensureEngineRunning: async () => ({ kind: "connected", lock: discoveries.shift()! }),
  ReconnectingEngineClient,
});
if (bunMock) bunMock.module("@makekosmos/ark", fakeArkModule);
else mock.module("@makekosmos/ark", fakeArkModule);

const { EngineClient } = await import("./host-api");

afterEach(() => {
  discoveries = [firstLock, secondLock];
  globalThis.fetch = originalFetch;
});

test("Host retries only its idempotent lifecycle settings request after 401", async () => {
  let calls = 0;
  const fetchMock = mockFn(async (_input: string) => {
    calls += 1;
    if (calls === 1) return new Response("", { status: 401 });
    return new Response(
      JSON.stringify({ ok: true, data: { desktop_host: { warm_timeout_seconds: 300 } } }),
    );
  });
  // SAFETY: Node's mock function has the same call signature as global fetch in this test.
  globalThis.fetch = fetchMock as typeof fetch;

  const client = new EngineClient("C:\\Kosmos-test");
  await expect(client.getWarmTimeout()).resolves.toEqual({ ok: true, data: 300 });
  expect(firstCallArgs(fetchMock)).toEqual([
    "http://127.0.0.1:4317/v1/rpc",
    "http://127.0.0.1:4318/v1/rpc",
  ]);
});

test("Host does not replay launch after 401 and uses the replacement for the following request", async () => {
  let calls = 0;
  const fetchMock = mockFn(async (_input: string) => {
    calls += 1;
    if (calls === 1) return new Response("", { status: 401 });
    return new Response(
      JSON.stringify({ ok: true, data: { desktop_host: { warm_timeout_seconds: 0 } } }),
    );
  });
  // SAFETY: Node's mock function has the same call signature as global fetch in this test.
  globalThis.fetch = fetchMock as typeof fetch;

  const client = new EngineClient("C:\\Kosmos-test");
  await expect(client.launchApp("demo")).resolves.toMatchObject({ ok: false });
  await expect(client.getWarmTimeout()).resolves.toEqual({ ok: true, data: 0 });
  expect(firstCallArgs(fetchMock)).toEqual([
    "http://127.0.0.1:4317/v1/apps/launch",
    "http://127.0.0.1:4318/v1/rpc",
  ]);
});

test("Host does not replay revoke after a closed transport and uses the replacement for the following request", async () => {
  let calls = 0;
  const fetchMock = mockFn(async (_input: string) => {
    calls += 1;
    if (calls === 1) throw new Error("transport closed");
    return new Response(
      JSON.stringify({ ok: true, data: { desktop_host: { warm_timeout_seconds: 300 } } }),
    );
  });
  // SAFETY: Node's mock function has the same call signature as global fetch in this test.
  globalThis.fetch = fetchMock as typeof fetch;

  const client = new EngineClient("C:\\Kosmos-test");
  await expect(client.revokeApp("123e4567-e89b-12d3-a456-426614174000")).resolves.toMatchObject({
    ok: false,
  });
  await expect(client.getWarmTimeout()).resolves.toEqual({ ok: true, data: 300 });
  expect(firstCallArgs(fetchMock)).toEqual([
    "http://127.0.0.1:4317/v1/apps/launch/123e4567-e89b-12d3-a456-426614174000",
    "http://127.0.0.1:4318/v1/rpc",
  ]);
});

test("Host registers a directory grant without returning the selected path", async () => {
  const fetchMock = mockFn(async (input: string, init?: RequestInit) => {
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
  // SAFETY: Node's mock function has the same call signature as global fetch in this test.
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
  const fetchMock = mockFn(async () => new Response("unexpected", { status: 500 }));
  // SAFETY: Node's mock function has the same call signature as global fetch in this test.
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
