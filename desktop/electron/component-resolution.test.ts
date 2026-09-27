import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { expect, mock, test } from "../test-support/node-test.mjs";

mock.module("electron", () => ({
  app: { isPackaged: true, getPath: () => os.tmpdir() },
  shell: { openPath: async () => "" },
}));
const { resolvePackagedManagerExecutable } = await import("./manager-navigation");
const { resolvePackagedAgendaExecutable } = await import("./agenda-navigation");
const { resolvePackagedMemoriaExecutable } = await import("./memoria-navigation");
const { resolvePackagedHostExecutable } = await import("./host-app");

test("packaged component resolvers stay Windows-only and require staged executables", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-components-"));
  const manager = path.join(root, "components", "manager");
  const agenda = path.join(root, "components", "agenda");
  const memoria = path.join(root, "components", "memoria");
  const host = path.join(root, "components", "host");
  fs.mkdirSync(manager, { recursive: true });
  fs.mkdirSync(agenda, { recursive: true });
  fs.mkdirSync(memoria, { recursive: true });
  fs.mkdirSync(host, { recursive: true });
  const managerExe = path.join(manager, "Kosmos Manager.exe");
  const agendaExe = path.join(agenda, "Kosmos Agenda.exe");
  const memoriaExe = path.join(memoria, "Kosmos Memoria.exe");
  const hostExe = path.join(host, "Kosmos Package Host.exe");
  fs.writeFileSync(managerExe, "fixture");
  fs.writeFileSync(agendaExe, "fixture");
  fs.writeFileSync(memoriaExe, "fixture");
  fs.writeFileSync(hostExe, "fixture");
  expect(resolvePackagedManagerExecutable(root, "win32")).toBe(managerExe);
  expect(resolvePackagedAgendaExecutable(root, "win32")).toBe(agendaExe);
  expect(resolvePackagedMemoriaExecutable(root, "win32")).toBe(memoriaExe);
  expect(resolvePackagedHostExecutable(root, "win32")).toBe(hostExe);
  expect(resolvePackagedManagerExecutable(root, "linux")).toBeNull();
  expect(resolvePackagedAgendaExecutable(root, "linux")).toBeNull();
  expect(resolvePackagedMemoriaExecutable(root, "linux")).toBeNull();
  fs.rmSync(root, { recursive: true, force: true });
});
