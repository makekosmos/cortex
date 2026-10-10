import assert from "node:assert/strict";
import { test } from "node:test";
import { fetchManifestUntilReady, manifestReadinessProblem } from "./verify-manifest-retry.mjs";

// Real v0.10.11 manifest.json shape (validateReleaseManifest is strict:
// asset urls must point at the v0.10.11 release, sha512 must be base64).
const BOTH = JSON.stringify({
  schema: "mundus-release-manifest",
  schema_version: 1,
  product: "mundus",
  channel: "production",
  version: "0.10.11",
  compatibility: { engine_api: "1.0.0" },
  source: {
    repository: "makekosmos/cortex",
    commit: "20ca8b89cca0aa0805e1a0bbdcb555ba846ead3f",
    toolchain: { pnpm: "12.4.1", node: "24.15.0", rust: "1.95.0" },
  },
  platforms: {
    win: {
      file: "Mundus-Setup-0.10.11.exe",
      url: "https://github.com/makekosmos/cortex/releases/download/v0.10.11/Mundus-Setup-0.10.11.exe",
      size: 40934457,
      sha512:
        "+uzj8alKqw0qRlJkglKUXRCUng7w6bzz2LyycjZS4xDjnCCtkQZJ4+9JwFZrUOaqiBL5JYX8nAtWAkM7ysgaWg==",
      target: "x86_64-pc-windows-msvc",
      commit: "20ca8b89cca0aa0805e1a0bbdcb555ba846ead3f",
      released_at: "2026-10-10T01:01:40.294Z",
    },
    mac: {
      file: "Mundus-0.10.11.dmg",
      url: "https://github.com/makekosmos/cortex/releases/download/v0.10.11/Mundus-0.10.11.dmg",
      size: 38596087,
      sha512:
        "axSlzuuBwFd97zFbviMJSIAp+kV+DWaVIyINT3ciKksnK8mAFuOPb/SFKJMhMbF87KnK7QcwkX8FN87t51IiyA==",
      target: "aarch64-apple-darwin",
      commit: "86b3361dd609347888fa9113f475a86533fe23aa",
      released_at: "2026-10-10T00:41:42.116Z",
    },
  },
});

// The stale copy the CDN served right after the mac publish: win only.
const WIN_ONLY = JSON.stringify({
  ...JSON.parse(BOTH),
  platforms: { win: JSON.parse(BOTH).platforms.win },
});

const BOTH_OLD_VERSION = JSON.stringify({
  ...JSON.parse(BOTH),
  version: "0.10.10",
});

function responses(bodies) {
  const queue = [...bodies];
  const calls = [];
  const fetchImpl = async (url) => {
    calls.push(url);
    const body = queue.length > 1 ? queue.shift() : queue[0];
    if (body === null) {
      return { ok: false, status: 404, statusText: "Not Found", text: async () => "" };
    }
    return { ok: true, status: 200, text: async () => body };
  };
  return { fetchImpl, calls };
}

const args = {
  url: "https://github.com/makekosmos/cortex/releases/download/v0.10.11/manifest.json",
  platform: "mac",
  version: "0.10.11",
  deadlineMs: 60_000,
  sleepImpl: async () => {},
};

test("readiness problem flags a missing platform entry and a version mismatch", () => {
  assert.match(manifestReadinessProblem(JSON.parse(WIN_ONLY), "mac", "0.10.11"), /no "mac" entry/);
  assert.match(
    manifestReadinessProblem(JSON.parse(BOTH_OLD_VERSION), "mac", "0.10.11"),
    /0\.10\.10 != expected 0\.10\.11/,
  );
  assert.equal(manifestReadinessProblem(JSON.parse(BOTH), "mac", "0.10.11"), null);
});

test("polls through a stale win-only manifest until the mac entry appears", async () => {
  const { fetchImpl, calls } = responses([WIN_ONLY, WIN_ONLY, BOTH]);
  const { manifest: found, attempt } = await fetchManifestUntilReady({
    ...args,
    fetchImpl,
  });
  assert.equal(attempt, 3);
  assert.equal(found.platforms.mac.file, "Mundus-0.10.11.dmg");
  assert.equal(calls.length, 3);
  for (const url of calls) assert.match(url, /[?&]cb=\d+-\d+/);
  assert.notEqual(calls[0], calls[1], "cache-buster must differ per attempt");
});

test("a real mismatch still fails once the deadline passes", async () => {
  const { fetchImpl, calls } = responses([WIN_ONLY]);
  let clock = 1_000;
  await assert.rejects(
    fetchManifestUntilReady({
      ...args,
      fetchImpl,
      deadlineMs: 10_000,
      now: () => clock,
      sleepImpl: async (ms) => {
        clock += ms;
      },
    }),
    /still not ready after 10s.*no "mac" entry/,
  );
  assert.ok(calls.length >= 2, "must have retried before giving up");
  assert.ok(clock >= 11_000, `deadline exceeded: ${clock}`);
  assert.ok(clock <= 11_000 + 20_000, `loop overshot the bound: ${clock}`);
});

test("network failures count as attempts and stop at the bound", async () => {
  const { fetchImpl, calls } = responses([null]);
  let clock = 0;
  await assert.rejects(
    fetchManifestUntilReady({
      ...args,
      fetchImpl,
      deadlineMs: 5_000,
      now: () => clock,
      sleepImpl: async (ms) => {
        clock += ms;
      },
    }),
    /still not ready after 5s.*HTTP 404/,
  );
  assert.ok(calls.length >= 2);
});

test("an already-ready manifest returns on the first attempt", async () => {
  const { fetchImpl, calls } = responses([BOTH]);
  const { attempt } = await fetchManifestUntilReady({ ...args, fetchImpl });
  assert.equal(attempt, 1);
  assert.equal(calls.length, 1);
});
