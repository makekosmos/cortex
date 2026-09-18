import { describe, expect, test } from "../test-support/node-test.mjs";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  USER_DATA_MAX_BYTES,
  type UserDataBackend,
  type UserDataCallResult,
  createEngineUserDataStores,
} from "./extension-user-data-ipc";

type BackendOverrides = Partial<UserDataBackend>;

function makeBackend(overrides: BackendOverrides = {}) {
  const calls: string[] = [];
  const files = new Map<string, Uint8Array>();
  const impls: UserDataBackend = {
    openRoot: () => Promise.resolve({ ok: true, data: { rootId: "root-1" } }),
    read: (_rootId, appId, key) => {
      const bytes = files.get(`${appId}/${key}`);
      return Promise.resolve(bytes ? { ok: true, data: bytes } : { ok: false, error: "not-found" });
    },
    write: (_rootId, appId, key, bytes) => {
      files.set(`${appId}/${key}`, bytes);
      return Promise.resolve({ ok: true, data: { sizeBytes: bytes.byteLength } });
    },
    stat: (rootId, appId, key) =>
      impls
        .read(rootId, appId, key)
        .then((result) =>
          result.ok ? { ok: true, data: { sizeBytes: result.data.byteLength } } : result,
        ),
    delete: (_rootId, appId, key) =>
      Promise.resolve(
        files.delete(`${appId}/${key}`)
          ? { ok: true, data: { deleted: true as const } }
          : { ok: false, error: "not-found" },
      ),
    ...overrides,
  };
  const backend: UserDataBackend = {
    openRoot: (root) => {
      calls.push(`openRoot:${root}`);
      return impls.openRoot(root);
    },
    read: (rootId, appId, key) => {
      calls.push(`read:${rootId}:${appId}:${key}`);
      return impls.read(rootId, appId, key);
    },
    write: (rootId, appId, key, bytes) => {
      calls.push(`write:${rootId}:${appId}:${key}`);
      return impls.write(rootId, appId, key, bytes);
    },
    stat: (rootId, appId, key) => {
      calls.push(`stat:${rootId}:${appId}:${key}`);
      return impls.stat(rootId, appId, key);
    },
    delete: (rootId, appId, key) => {
      calls.push(`delete:${rootId}:${appId}:${key}`);
      return impls.delete(rootId, appId, key);
    },
  };
  return { backend, calls, files };
}

describe("engine user data stores", () => {
  test("pins the root once and reuses it across apps and operations", async () => {
    const { backend, calls } = makeBackend();
    const stores = createEngineUserDataStores(backend, "C:\\host\\userData");
    const agenda = stores.forApp("com.kosmos.agenda");
    const notes = stores.forApp("com.kosmos.notes");
    await agenda.write("a.bin", new Uint8Array([1]));
    await notes.write("b.bin", new Uint8Array([2]));
    await agenda.read("a.bin");
    await notes.stat("b.bin");
    expect(calls.filter((call) => call.startsWith("openRoot"))).toEqual([
      "openRoot:C:\\host\\userData",
    ]);
    expect(calls).toContain("write:root-1:com.kosmos.agenda:a.bin");
    expect(calls).toContain("write:root-1:com.kosmos.notes:b.bin");
    expect(calls).toContain("read:root-1:com.kosmos.agenda:a.bin");
    expect(calls).toContain("stat:root-1:com.kosmos.notes:b.bin");
  });

  test("round-trips bytes with stat and delete through the backend", async () => {
    const { backend } = makeBackend();
    const store = createEngineUserDataStores(backend, "root").forApp("com.kosmos.agenda");
    expect(await store.read("missing.bin")).toEqual({ ok: false, error: "not-found" });
    expect(await store.stat("missing.bin")).toEqual({ ok: false, error: "not-found" });
    expect(await store.write("attachments/task-1.bin", new Uint8Array([0, 1, 255]))).toEqual({
      ok: true,
      data: { sizeBytes: 3 },
    });
    expect(await store.read("attachments/task-1.bin")).toEqual({
      ok: true,
      data: new Uint8Array([0, 1, 255]),
    });
    expect(await store.stat("attachments/task-1.bin")).toEqual({
      ok: true,
      data: { sizeBytes: 3 },
    });
    expect(await store.delete("attachments/task-1.bin")).toEqual({ ok: true, data: true });
    expect(await store.delete("attachments/task-1.bin")).toEqual({
      ok: false,
      error: "not-found",
    });
  });

  test("re-pins and retries once when Engine reports unknown-root", async () => {
    let next = 0;
    const { backend, calls, files } = makeBackend({
      openRoot: () => Promise.resolve({ ok: true, data: { rootId: `root-${++next}` } }),
      read: (rootId, appId, key) =>
        Promise.resolve(
          rootId === "root-1"
            ? { ok: false, error: "unknown-root" }
            : { ok: true, data: files.get(`${appId}/${key}`)! },
        ),
    });
    files.set("app/a.bin", new Uint8Array([7]));
    const store = createEngineUserDataStores(backend, "root").forApp("app");
    expect(await store.read("a.bin")).toEqual({ ok: true, data: new Uint8Array([7]) });
    expect(calls).toEqual([
      "openRoot:root",
      "read:root-1:app:a.bin",
      "openRoot:root",
      "read:root-2:app:a.bin",
    ]);
  });

  test("reports io-error when the root stays unknown after re-pin", async () => {
    const { backend, calls } = makeBackend({
      read: () => Promise.resolve({ ok: false, error: "unknown-root" }),
    });
    const store = createEngineUserDataStores(backend, "root").forApp("app");
    expect(await store.read("a.bin")).toEqual({ ok: false, error: "io-error" });
    expect(calls.filter((call) => call.startsWith("openRoot"))).toHaveLength(2);
    expect(calls.filter((call) => call.startsWith("read"))).toHaveLength(2);
  });

  test("does not re-pin for errors other than unknown-root", async () => {
    const { backend, calls } = makeBackend({
      read: () => Promise.resolve({ ok: false, error: "io-error" }),
    });
    const store = createEngineUserDataStores(backend, "root").forApp("app");
    expect(await store.read("a.bin")).toEqual({ ok: false, error: "io-error" });
    expect(calls.filter((call) => call.startsWith("openRoot"))).toHaveLength(1);
  });

  test("recovers from a transient openRoot failure", async () => {
    let failures = 1;
    const { backend } = makeBackend({
      openRoot: () =>
        Promise.resolve(
          failures-- > 0
            ? { ok: false, error: "unavailable" }
            : { ok: true, data: { rootId: "root-1" } },
        ),
    });
    const store = createEngineUserDataStores(backend, "root").forApp("app");
    expect(await store.read("a.bin")).toEqual({ ok: false, error: "io-error" });
    expect(await store.write("a.bin", new Uint8Array([1]))).toEqual({
      ok: true,
      data: { sizeBytes: 1 },
    });
  });

  test("creates the root directory when Engine reports it missing", async () => {
    const base = mkdtempSync(path.join(os.tmpdir(), "kosmos-user-data-"));
    const root = path.join(base, "userData");
    try {
      const { backend, calls } = makeBackend({
        openRoot: (target) =>
          Promise.resolve(
            existsSync(target)
              ? { ok: true, data: { rootId: "root-1" } }
              : { ok: false, error: "not-found" },
          ),
      });
      const store = createEngineUserDataStores(backend, root).forApp("app");
      expect(await store.stat("a.bin")).toEqual({ ok: false, error: "not-found" });
      expect(existsSync(root)).toBe(true);
      expect(calls.filter((call) => call.startsWith("openRoot"))).toHaveLength(2);
    } finally {
      rmSync(base, { recursive: true, force: true });
    }
  });

  test("rejects malformed root ids and malformed app ids", async () => {
    const { backend } = makeBackend({
      openRoot: () => Promise.resolve({ ok: true, data: { rootId: "../escape" } }),
    });
    const stores = createEngineUserDataStores(backend, "root");
    expect(await stores.forApp("app").read("a.bin")).toEqual({ ok: false, error: "io-error" });

    const { backend: healthy, calls } = makeBackend();
    const healthyStores = createEngineUserDataStores(healthy, "root");
    for (const appId of ["bad app", "..", "a/b", "C:\\x"]) {
      expect(await healthyStores.forApp(appId).read("a.bin")).toEqual({
        ok: false,
        error: "io-error",
      });
    }
    expect(calls).toHaveLength(0);
  });

  test("maps engine error codes to the typed contract", async () => {
    for (const [code, expected] of [
      ["not-found", "not-found"],
      ["invalid-key", "invalid-key"],
      ["too-large", "too-large"],
      ["io-error", "io-error"],
      ["unavailable", "io-error"],
      ["invalid-request", "io-error"],
      ["forbidden", "io-error"],
      ["unexpected", "io-error"],
    ] as const) {
      const { backend } = makeBackend({
        read: () => Promise.resolve({ ok: false, error: code }),
      });
      const store = createEngineUserDataStores(backend, "root").forApp("app");
      expect(await store.read("a.bin")).toEqual({ ok: false, error: expected });
    }
  });

  test("validates keys and payload sizes before touching Engine", async () => {
    const { backend, calls } = makeBackend();
    const store = createEngineUserDataStores(backend, "root").forApp("app");
    for (const key of ["../escape", "/absolute", "C:\\escape", "", "a//b"]) {
      expect(await store.read(key)).toEqual({ ok: false, error: "invalid-key" });
      expect(await store.write(key, new Uint8Array([1]))).toEqual({
        ok: false,
        error: "invalid-key",
      });
      expect(await store.delete(key)).toEqual({ ok: false, error: "invalid-key" });
      expect(await store.stat(key)).toEqual({ ok: false, error: "invalid-key" });
    }
    expect(await store.write("large.bin", new Uint8Array(USER_DATA_MAX_BYTES + 1))).toEqual({
      ok: false,
      error: "too-large",
    });
    // SAFETY: a JSON boundary stands in for a renderer sending a non-binary payload.
    expect(await store.write("a.bin", JSON.parse('"text"'))).toEqual({
      ok: false,
      error: "io-error",
    });
    expect(calls).toHaveLength(0);
  });

  test("rejects malformed or oversized backend payloads", async () => {
    const oversizedRead = makeBackend({
      read: () =>
        Promise.resolve({
          ok: true,
          data: new Uint8Array(USER_DATA_MAX_BYTES + 1),
        } satisfies UserDataCallResult<Uint8Array>),
    });
    expect(
      await createEngineUserDataStores(oversizedRead.backend, "root").forApp("app").read("a.bin"),
    ).toEqual({ ok: false, error: "too-large" });

    const wrongSize = makeBackend({
      write: () =>
        Promise.resolve({
          ok: true,
          data: { sizeBytes: 2 },
        } satisfies UserDataCallResult<{ sizeBytes: number }>),
    });
    expect(
      await createEngineUserDataStores(wrongSize.backend, "root")
        .forApp("app")
        .write("a.bin", new Uint8Array([1])),
    ).toEqual({ ok: false, error: "io-error" });

    const undeleted: UserDataCallResult<{ deleted: boolean }> = {
      ok: true,
      data: { deleted: false },
    };
    const lyingDelete = makeBackend({
      // SAFETY: fabricates a malformed wire payload the backend contract forbids.
      delete: () => Promise.resolve(undeleted as UserDataCallResult<{ deleted: true }>),
    });
    expect(
      await createEngineUserDataStores(lyingDelete.backend, "root").forApp("app").delete("a.bin"),
    ).toEqual({ ok: false, error: "io-error" });

    for (const sizeBytes of [-1, 1.5, USER_DATA_MAX_BYTES + 1]) {
      const badStat = makeBackend({
        stat: () =>
          Promise.resolve({
            ok: true,
            data: { sizeBytes },
          } satisfies UserDataCallResult<{ sizeBytes: number }>),
      });
      expect(
        await createEngineUserDataStores(badStat.backend, "root").forApp("app").stat("a.bin"),
      ).toEqual({ ok: false, error: sizeBytes > USER_DATA_MAX_BYTES ? "too-large" : "io-error" });
    }
  });
});
