// System service client — общается с packaged `Kosmos System Service.exe`
// (LocalSystem Windows Service) через named pipe `\\.\pipe\kosmos-system-service`.
//
// Service устанавливается опционально (Settings → Focus → "Установить
// daemon"). Если установлен — все hosts модификации идут через pipe
// (без UAC на каждую активацию). Если не установлен — fallback на
// `Kosmos Helper.exe` spawn с UAC elevation (см. focus-block.ts).

import { spawn } from "node:child_process";
import net from "node:net";
import path from "node:path";
import { app } from "electron";
import { fileURLToPath } from "node:url";
import { isRecord, isString } from "../src/shared/runtimeGuards";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PIPE_PATH = "\\\\.\\pipe\\kosmos-system-service";
const LEGACY_PIPE_PATH = "\\\\.\\pipe\\kepler-focus-svc";

interface ServiceRequest {
  op: "add" | "remove" | "reset" | "status" | "ping";
  domains?: string[];
}

interface ServiceResponse {
  ok: boolean;
  active_domains?: string[];
  error?: string;
  pong?: boolean;
}

function servicePath(): string {
  if (app.isPackaged) {
    return path.join(process.resourcesPath, "Kosmos System Service.exe");
  }
  return path.resolve(__dirname, "..", "..", "..", "target", "release", "kepler-focus-svc.exe");
}

// --- CLI invocations (install / uninstall / status) ------------------------

interface CliResult {
  ok: boolean;
  installed?: boolean;
  running?: boolean;
  service_name?: string;
  needs_elevation?: boolean;
  error?: string;
}

function isServiceResponse<T>(value: T): value is T & ServiceResponse {
  return (
    isRecord(value) &&
    typeof value.ok === "boolean" &&
    (value.active_domains === undefined ||
      (Array.isArray(value.active_domains) &&
        value.active_domains.every((domain) => isString(domain)))) &&
    (value.error === undefined || typeof value.error === "string") &&
    (value.pong === undefined || typeof value.pong === "boolean")
  );
}

function isCliResult<T>(value: T): value is T & CliResult {
  return (
    isRecord(value) &&
    typeof value.ok === "boolean" &&
    (value.installed === undefined || typeof value.installed === "boolean") &&
    (value.running === undefined || typeof value.running === "boolean") &&
    (value.service_name === undefined || typeof value.service_name === "string") &&
    (value.needs_elevation === undefined || typeof value.needs_elevation === "boolean") &&
    (value.error === undefined || typeof value.error === "string")
  );
}

function parseCliResult(text: string): CliResult {
  const parsed: unknown = JSON.parse(text);
  if (isCliResult(parsed)) return parsed;
  return { ok: false, error: "invalid cli response shape" };
}

function parseServiceResponse(text: string): ServiceResponse {
  const parsed: unknown = JSON.parse(text);
  if (isServiceResponse(parsed)) return parsed;
  return { ok: false, error: "invalid service response shape" };
}

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
export async function getServiceStatus(): Promise<{ installed: boolean; running: boolean }> {
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

// --- Named pipe IPC --------------------------------------------------------

/** Send single JSON request to running service via named pipe.
    Returns response or error если pipe не доступен (service не запущен). */
function sendViaPipePath(pipePath: string, req: ServiceRequest): Promise<ServiceResponse> {
  return new Promise((resolve) => {
    let resolved = false;
    const finish = (resp: ServiceResponse) => {
      if (!resolved) {
        resolved = true;
        resolve(resp);
      }
    };

    let buffer = "";
    const client = net.connect(pipePath);

    client.on("connect", () => {
      try {
        client.write(JSON.stringify(req) + "\n");
      } catch (e) {
        // SAFETY: Node event handlers provide Error-like failures here.
        finish({ ok: false, error: `pipe write: ${(e as Error).message}` });
        client.destroy();
      }
    });

    client.on("data", (chunk: Buffer) => {
      buffer += chunk.toString("utf8");
      // Service отвечает одним JSON и закрывает соединение. Пробуем
      // распарсить как только видим что-то целое.
      try {
        const parsed = parseServiceResponse(buffer.trim());
        finish(parsed);
        client.end();
      } catch {
        // Возможно ещё не всё пришло — ждём 'end'.
      }
    });

    client.on("end", () => {
      if (!resolved) {
        try {
          finish(parseServiceResponse(buffer.trim()));
        } catch (e) {
          // SAFETY: JSON parsing failures are Error instances in this boundary.
          finish({ ok: false, error: `pipe parse: ${(e as Error).message}` });
        }
      }
    });

    client.on("error", (e: NodeJS.ErrnoException) => {
      // ENOENT = service не запущен. EACCES = pipe ACL отказал. ETIMEDOUT = таймаут.
      finish({ ok: false, error: `pipe ${e.code ?? ""}: ${e.message}` });
    });

    // Safety timeout (3s)
    setTimeout(() => finish({ ok: false, error: "pipe timeout" }), 3000);
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
