import fs, { readFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterAll, expect, mock, test } from "../test-support/node-test.mjs";

const appData = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-desktop-host-app-"));
const startMenuFolder = "Kosmos";
const openedShortcuts: string[] = [];
let openPathMode: "ok" | "error" | "reject" = "ok";
const realFs = fs;
afterAll(() => realFs.rmSync(appData, { recursive: true, force: true }));
mock.module("node:fs", () => ({
  default: {
    mkdirSync: realFs.mkdirSync.bind(realFs),
    mkdtempSync: realFs.mkdtempSync.bind(realFs),
    writeFileSync: realFs.writeFileSync.bind(realFs),
    existsSync: realFs.existsSync.bind(realFs),
    statSync: realFs.statSync.bind(realFs),
  },
}));
mock.module("electron", () => ({
  app: { isPackaged: true, getPath: () => appData },
  shell: {
    openPath: mock(async (target: string) => {
      openedShortcuts.push(target);
      if (openPathMode === "reject") throw new Error("open failed");
      return openPathMode === "error" ? "open failed" : "";
    }),
  },
}));

const { openHostedApp } = await import("./host-app");

test("My cosmos command routes through the optional Desktop Host", () => {
  const source = readFileSync(new URL("./commands.ts", import.meta.url), "utf8");
  expect(source).toContain('id: "kosmos:my-cosmos"');
  expect(source).toContain('openHostedApp("com.kosmos.graph")');
  expect(source).not.toContain("openMyCosmosWindow");
});

test("packaged Desktop opens the Host-owned Start Menu shortcut", async () => {
  delete process.env.KOSMOS_HOST_EXECUTABLE;
  delete process.env.KOSMOS_HOST_MAIN;
  openPathMode = "ok";
  openedShortcuts.length = 0;
  const root = path.join(
    appData,
    "Microsoft",
    "Windows",
    "Start Menu",
    "Programs",
    startMenuFolder,
  );
  fs.mkdirSync(root, { recursive: true });
  const shortcut = path.join(root, "com.kosmos.graph.lnk");
  fs.writeFileSync(shortcut, "test shortcut");

  await openHostedApp("com.kosmos.graph");

  expect(openedShortcuts).toEqual([shortcut]);
});

test("packaged Desktop rejects unsafe or missing Host shortcuts", async () => {
  delete process.env.KOSMOS_HOST_EXECUTABLE;
  delete process.env.KOSMOS_HOST_MAIN;
  openedShortcuts.length = 0;

  await openHostedApp("../escape");
  await openHostedApp("com.kosmos.missing");

  expect(openedShortcuts).toEqual([]);
});

test("packaged Desktop handles shortcut open errors", async () => {
  delete process.env.KOSMOS_HOST_EXECUTABLE;
  delete process.env.KOSMOS_HOST_MAIN;
  const root = path.join(
    appData,
    "Microsoft",
    "Windows",
    "Start Menu",
    "Programs",
    startMenuFolder,
  );
  const shortcut = path.join(root, "com.kosmos.graph.lnk");
  fs.writeFileSync(shortcut, "test shortcut");

  for (const mode of ["error", "reject"] as const) {
    openPathMode = mode;
    await expect(openHostedApp("com.kosmos.graph")).resolves.toBeUndefined();
  }
});
