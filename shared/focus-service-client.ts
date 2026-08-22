// System service client — общается с packaged `Kosmos System Service.exe`
// (LocalSystem Windows Service) через named pipe `\\.\pipe\kosmos-system-service`.
//
// Service устанавливается опционально (Settings → Focus → "Установить
// daemon"). Если установлен — все hosts модификации идут через pipe
// (без UAC на каждую активацию). Если не установлен — fallback на
// `Kosmos Helper.exe` spawn с UAC elevation (см. focus-block.ts).

import { spawn } from "node:child_process";
import { app } from "electron";
import {
  parseCliResult,
  resolveFocusServicePath,
  sendViaPipePath,
  type FocusServiceCliResult,
  type FocusServiceResponse,
  type ServiceRequest,
} from "./focus-service-protocol";
export {
  parseCliResult,
  parseServiceResponse,
  resolveFocusServicePath,
  sendViaPipePath,
} from "./focus-service-protocol";

const PIPE_PATH = "\\\\.\\pipe\\kosmos-system-service";
const LEGACY_PIPE_PATH = "\\\\.\\pipe\\kepler-focus-svc";

type ServiceResponse = FocusServiceResponse;

function servicePath(): string {
  return resolveFocusServicePath({
    packaged: app.isPackaged,
    resourcesPath: process.resourcesPath,
    appPath: app.getAppPath(),
  });
}

// --- CLI invocations (install / uninstall / status) ------------------------

type CliResult = FocusServiceCliResult;

function runCli(args: string[]): Promise<CliResult> {
  return new Promise((resolve) => {
    let stdout = "";
    let stderr = "";
    const child = spawn(servicePath(), args, {
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    child.stdout?.on("data", (c: Buffer) => (stdout += c.toString("utf8")));
    child.stderr?.on("data", (c: Buffer) => (stderr += c.toString("utf8")));
    child.on("error", (e) => resolve({ ok: false, error: e.message }));
    child.on("close", () => {
      const trimmed = stdout.trim();
      if (!trimmed) {
        resolve({ ok: false, error: stderr.slice(0, 200) });
        return;
      }
      try {
        resolve(parseCliResult(trimmed));
      } catch {
        resolve({ ok: false, error: `unparseable: ${trimmed.slice(0, 200)}` });
      }
    });
  });
}

/** Status работает user-mode (читает SCM без admin). */
export async function getServiceStatus(): Promise<{
  installed: boolean;
  running: boolean;
}> {
  const r = await runCli(["status"]);
  return { installed: !!r.installed, running: !!r.running };
}

/** Install/uninstall с elevation via PowerShell RunAs. Используется UI кнопками. */
export function runServiceCliElevated(
  subcommand: string,
): Promise<{ ok: boolean; error?: string }> {
  return new Promise((resolve) => {
    const bin = servicePath();
    const psCommand = [
      "Start-Process",
      "-FilePath",
      `'${bin.replace(/'/g, "''")}'`,
      "-ArgumentList",
      `'${subcommand}'`,
      "-Verb",
      "RunAs",
      "-WindowStyle",
      "Hidden",
      "-Wait",
    ].join(" ");

    const ps = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", psCommand], {
      windowsHide: true,
    });
    let stderr = "";
    ps.stderr?.on("data", (c) => (stderr += c.toString("utf8")));
    ps.on("error", (e) => resolve({ ok: false, error: e.message }));
    ps.on("close", (code) => {
      if (code === 0) resolve({ ok: true });
      else
        resolve({
          ok: false,
          error: `PowerShell exited ${code}: ${stderr.slice(0, 200)}`,
        });
    });
  });
}

export async function sendViaPipe(req: ServiceRequest): Promise<ServiceResponse> {
  const primary = await sendViaPipePath(PIPE_PATH, req);
  if (primary.ok || !primary.error?.includes("ENOENT")) return primary;
  return sendViaPipePath(LEGACY_PIPE_PATH, req);
}

/** Ping the service to verify it's running and pipe is healthy. */
export async function pingService(): Promise<boolean> {
  const resp = await sendViaPipe({ op: "ping" });
  return resp.ok === true && resp.pong === true;
}
