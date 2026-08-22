import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { expect, mock, test } from "bun:test";
import { resolvePackagedManagerExecutable } from "./manager-navigation";

mock.module("electron", () => ({
  app: { isPackaged: true, getPath: () => os.tmpdir() },
  shell: { openPath: async () => "" },
}));
const { resolvePackagedHostExecutable } = await import("./host-app");

test("packaged component resolvers stay Windows-only and require staged executables", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-components-"));
  const manager = path.join(root, "components", "manager");
  const host = path.join(root, "components", "host");
  fs.mkdirSync(manager, { recursive: true });
  fs.mkdirSync(host, { recursive: true });
  const managerExe = path.join(manager, "Kosmos Manager.exe");
  const hostExe = path.join(host, "Kosmos Package Host.exe");
  fs.writeFileSync(managerExe, "fixture");
  fs.writeFileSync(hostExe, "fixture");
  expect(resolvePackagedManagerExecutable(root, "win32")).toBe(managerExe);
  expect(resolvePackagedHostExecutable(root, "win32")).toBe(hostExe);
  expect(resolvePackagedManagerExecutable(root, "linux")).toBeNull();
  fs.rmSync(root, { recursive: true, force: true });
});
