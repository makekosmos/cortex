import { expect, test } from "bun:test";
import { spawn } from "node:child_process";
import type { ElectronApplication } from "playwright";
import { closeHost } from "./host-runtime";

test("closing an already exited Host is idempotent", async () => {
  const child = spawn(process.execPath, ["-e", ""], { windowsHide: true });
  await new Promise<void>((resolve) => child.once("exit", () => resolve()));
  const host = { process: () => child };
  // SAFETY: closeHost only reads process() when the supplied child has already exited.
  const exitedHost = host as ElectronApplication;
  await closeHost(exitedHost, new Set());
  await closeHost(exitedHost, new Set());
  expect(child.exitCode).toBe(0);
});
