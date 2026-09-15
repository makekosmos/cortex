import { describe, expect, test } from "../test-support/node-test.mjs";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  AUTOSTART_ARGS,
  AUTOSTART_NAME,
  autostartExeCandidates,
  ENGINE_AUTOSTART_NAME,
  engineAutostartExe,
  isAutostartEnabled,
  setAutostartEnabled,
  type LaunchItemLike,
  type LoginItemApi,
} from "./main-autostart";

// In-memory model of the Windows Run key: values are keyed by name, a set
// write stores `path` + `args`, and StartupApproved can disable a name.
class FakeRunKey implements LoginItemApi {
  entries = new Map<string, { path: string; args: string[] }>();
  disabled = new Set<string>();

  getLoginItemSettings(options: { path: string; args: string[] }) {
    const wanted = path.normalize(options.path).toLowerCase();
    const launchItems: LaunchItemLike[] = [];
    for (const [name, entry] of this.entries) {
      if (path.normalize(entry.path).toLowerCase() !== wanted) continue;
      launchItems.push({
        name,
        path: entry.path,
        args: entry.args,
        enabled: !this.disabled.has(name),
      });
    }
    return { launchItems };
  }

  setLoginItemSettings(settings: {
    openAtLogin: boolean;
    name?: string;
    path?: string;
    args?: string[];
  }) {
    const name = settings.name ?? "com.kosmos.manager";
    if (settings.openAtLogin) {
      this.entries.set(name, { path: settings.path ?? "", args: settings.args ?? [] });
      this.disabled.delete(name);
    } else {
      this.entries.delete(name);
      this.disabled.delete(name);
    }
  }
}

const APP_EXE = path.join("C:", "Programs", "Kosmos", "Kosmos.exe");
const ENGINE_EXE = path.join("C:", "Engine", "versions", "0.1.3", "kepler-backend.exe");

describe("Manager autostart registration", () => {
  test("enable writes a durable 'Kosmos' Run entry and reads it back", () => {
    const api = new FakeRunKey();
    expect(setAutostartEnabled(api, APP_EXE, ENGINE_EXE, true)).toBe(true);
    expect(api.entries.get(AUTOSTART_NAME)).toEqual({ path: APP_EXE, args: AUTOSTART_ARGS });
    expect(isAutostartEnabled(api, APP_EXE, ENGINE_EXE)).toBe(true);
  });

  test("disable removes the entry and reports the real state", () => {
    const api = new FakeRunKey();
    setAutostartEnabled(api, APP_EXE, ENGINE_EXE, true);
    expect(setAutostartEnabled(api, APP_EXE, ENGINE_EXE, false)).toBe(false);
    expect(api.entries.size).toBe(0);
    expect(isAutostartEnabled(api, APP_EXE, ENGINE_EXE)).toBe(false);
  });

  test("enabling clears legacy names and a stray engine entry once", () => {
    const api = new FakeRunKey();
    api.entries.set("CosCast", { path: "C:\\old\\CosCast.exe", args: AUTOSTART_ARGS });
    api.entries.set("KosmosOld", { path: APP_EXE, args: AUTOSTART_ARGS });
    api.entries.set(ENGINE_AUTOSTART_NAME, { path: ENGINE_EXE, args: ["--start"] });
    expect(setAutostartEnabled(api, APP_EXE, ENGINE_EXE, true)).toBe(true);
    expect([...api.entries.keys()]).toEqual([AUTOSTART_NAME]);
  });

  test("an existing engine autostart still reports enabled", () => {
    const api = new FakeRunKey();
    api.entries.set(ENGINE_AUTOSTART_NAME, { path: ENGINE_EXE, args: ["--start"] });
    expect(isAutostartEnabled(api, APP_EXE, ENGINE_EXE)).toBe(true);
  });

  test("an entry disabled in StartupApproved reports disabled", () => {
    const api = new FakeRunKey();
    api.entries.set(AUTOSTART_NAME, { path: APP_EXE, args: AUTOSTART_ARGS });
    api.disabled.add(AUTOSTART_NAME);
    expect(isAutostartEnabled(api, APP_EXE, ENGINE_EXE)).toBe(false);
    expect(setAutostartEnabled(api, APP_EXE, ENGINE_EXE, true)).toBe(true);
    expect(api.disabled.has(AUTOSTART_NAME)).toBe(false);
  });

  test("a legacy install path still counts as autostart", () => {
    const api = new FakeRunKey();
    const legacyExe = path.join(path.dirname(APP_EXE), "CosCast.exe");
    api.entries.set("CosCast", { path: legacyExe, args: AUTOSTART_ARGS });
    expect(isAutostartEnabled(api, APP_EXE, ENGINE_EXE)).toBe(true);
  });
});

describe("autostart path resolution", () => {
  test("engineAutostartExe follows the installed Engine pointer", () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-engine-"));
    const versionRoot = path.join(root, "Kosmos", "Engine", "versions", "1.2.3");
    mkdirSync(versionRoot, { recursive: true });
    const backend = path.join(versionRoot, "kepler-backend.exe");
    writeFileSync(backend, "fixture");
    writeFileSync(
      path.join(root, "Kosmos", "Engine", "current.json"),
      JSON.stringify({ schema_version: 1, version: "1.2.3" }),
    );
    expect(engineAutostartExe(APP_EXE, root)).toBe(
      path.join(root, "Kosmos", "Engine", "versions", "1.2.3", "kepler-backend.exe"),
    );
    expect(engineAutostartExe(APP_EXE, path.join(root, "missing"))).toBe(
      path.join(path.dirname(APP_EXE), "resources", "Kosmos Runtime.exe"),
    );
  });

  test("autostartExeCandidates covers current and legacy install locations once", () => {
    const localAppData = path.join("C:", "Users", "me", "AppData", "Local");
    const candidates = autostartExeCandidates(APP_EXE, localAppData);
    expect(candidates).toContain(APP_EXE.toLowerCase());
    expect(candidates).toContain(path.join(path.dirname(APP_EXE), "CosCast.exe").toLowerCase());
    expect(candidates).toContain(
      path.resolve(localAppData, "Programs", "Kosmos", "Kosmos.exe").toLowerCase(),
    );
    expect(new Set(candidates).size).toBe(candidates.length);
  });
});
