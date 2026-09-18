// open-note scenario steps: package install, object seeding, process cleanup.
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { processInfo, sameProcessIdentity, stopProcessTree } from "../dev-run-process.mjs";
import { delay, engineRpc, engineShutdown, iso, processesUnder, waitPidGone } from "./lib.mjs";
import { AGENDA_ID, MEMORIA_ID, noteObject, taskNoteLink, taskObject } from "./open-note-data.mjs";

export async function installPackages(lock, fixture, record) {
  // Install + enable Agenda and Memoria from the pinned .kspkg archives.
  for (const [id, info] of Object.entries(fixture.packages)) {
    const install = await engineRpc(lock, "packages.install", {
      id,
      version: info.version,
      archive_path: info.kspkg,
    });
    record(`packages.install ${id}`, () => {
      if (!install.ok) throw new Error(JSON.stringify(install.error));
    });
    const enable = await engineRpc(lock, "packages.set_enabled", {
      id,
      version: info.version,
      enabled: true,
    });
    record(`packages.set_enabled ${id}`, () => {
      if (!enable.ok) throw new Error(JSON.stringify(enable.error));
    });
  }
  const list = await engineRpc(lock, "packages.list");
  record("packages.list shows both enabled", () => {
    if (!list.ok) throw new Error(JSON.stringify(list.error));
    const items = Array.isArray(list.data) ? list.data : (list.data?.packages ?? []);
    for (const id of [AGENDA_ID, MEMORIA_ID]) {
      const entry = items.find((item) => item.id === id);
      if (!entry) throw new Error(`${id} not installed`);
      if (entry.enabled === false) throw new Error(`${id} not enabled`);
    }
  });
}

export async function seedObjects(lock, record) {
  // Seed note A, note B, a task and the task->note link through the Engine
  // RPC directly (same ops the e2e suites use; no app window needed and no
  // manifest grant dependency).
  const now = iso();
  const noteA = `note:${crypto.randomUUID()}`;
  const noteB = `note:${crypto.randomUUID()}`;
  const taskId = `task:${crypto.randomUUID()}`;
  const titleA = "KOS-58 fixture note A";
  const titleB = "KOS-58 fixture note B";
  const seedOps = [
    [`upsert_object noteA ${noteA}`, { object: noteObject(noteA, titleA, now) }],
    [`upsert_object noteB ${noteB}`, { object: noteObject(noteB, titleB, now) }],
    [`upsert_object task ${taskId}`, { object: taskObject(taskId, "KOS-58 fixture task", now) }],
    [`upsert_object_link ${taskId} -> ${noteA}`, { object_link: taskNoteLink(taskId, noteA, now) }],
  ];
  for (const [label, params] of seedOps) {
    const [op] = label.split(" ");
    const seeded = await engineRpc(lock, op, { ...params, device_id: "kos58-fixture" });
    record(`seed ${label}`, () => {
      if (!seeded.ok) throw new Error(JSON.stringify(seeded.error));
    });
  }
  return { noteA, noteB, taskId, titleA, titleB };
}

// Engine launch URLs are http://127.0.0.1:<port>/v1/apps/assets/<token>/<entrypoint>
// — they carry no app id. Identify app windows by the preload-exposed
// window.kosmosApp.identity.id instead.
export const findPage = (browser) => async (appId) => {
  for (const page of browser.contexts().flatMap((context) => context.pages())) {
    try {
      const id = await page.evaluate(() => globalThis.kosmosApp?.identity?.id ?? null);
      if (id === appId) return page;
    } catch {}
  }
  return undefined;
};

// Graceful host close, then engine shutdown, then identity-checked kills.
// ctx holds mutable handles ({engine, host, browser}) the caller assigns.
export function makeCleanup({ ctx, result, check, spawned, engineBackend, dataDir, processRoots }) {
  return async () => {
    try {
      await ctx.browser?.close();
    } catch {}
    if (ctx.host?.child?.pid) {
      const tracked = spawned.find((item) => item.pid === ctx.host.child.pid);
      try {
        stopProcessTree(
          ctx.host.child.pid,
          tracked?.identity?.startTime,
          tracked?.identity?.commandLine,
          "host",
        );
      } catch {}
    }
    engineShutdown(engineBackend, dataDir);
    if (ctx.engine?.child?.pid) await waitPidGone(ctx.engine.child.pid, "Engine").catch(() => {});
    for (const item of spawned) {
      const current = processInfo(item.pid);
      if (current && sameProcessIdentity(current, item.identity)) {
        try {
          stopProcessTree(item.pid, item.identity.startTime, item.identity.commandLine, item.label);
        } catch {}
      }
    }
    // Drain: supervisor children (core worker, ark-core-rpc) can outlive the
    // tracked parent pid briefly; poll the fixture roots and force-kill any
    // process still anchored there using its live identity.
    const deadline = Date.now() + 15_000;
    let leftovers = processesUnder(processRoots);
    while (leftovers.length && Date.now() < deadline) {
      for (const leftover of leftovers) {
        const current = processInfo(leftover.ProcessId);
        if (current) {
          try {
            stopProcessTree(leftover.ProcessId, current.startTime, current.commandLine, "fixture");
          } catch {}
        }
      }
      await delay(300);
      leftovers = processesUnder(processRoots);
    }
    result.cleanup = {
      engine_lock_removed: !fs.existsSync(path.join(dataDir, "engine.lock.json")),
      leftover_processes: leftovers,
      roots: processRoots,
    };
    check(
      "cleanup: no leftover fixture processes",
      leftovers.length === 0,
      leftovers.map((p) => p.ExecutablePath).join("; ") || undefined,
    );
    check("cleanup: engine lock removed", result.cleanup.engine_lock_removed);
  };
}
