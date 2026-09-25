// Focus block applier — спавнит `Kosmos Helper.exe` для модификации
// hosts файла когда юзер активирует/деактивирует focus blocklist.
//
// Elevation flow:
//   Helper bin имеет `requireAdministrator` manifest. Если Kepler уже
//   запущен от админа → direct spawn + stdin pipe (быстро, тихо). Если
//   Kepler НЕ-админ → CreateProcess fails ERROR_ELEVATION_REQUIRED (740);
//   тогда фоллбэк на PowerShell `Start-Process -Verb RunAs` который
//   triggers UAC prompt и запускает helper из новой shell. UAC даёт нам
//   stdin недоступен → передаём request через `--input <file>` арг,
//   читаем response из `--output <file>`. Cleanup temp файлов.
//
// после успешного `focus.set_active_state` вызывает `applyFocusBlock(...)`.

import { spawn } from "node:child_process";
import { promises as fsp } from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";
import { app, BrowserWindow } from "electron";
import { fileURLToPath } from "node:url";
import { pingService, sendViaPipe } from "../../shared/focus-service-client";
import type { JsonRecord, JsonValue } from "./json-types";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

interface HelperRequest {
  op: "add" | "remove" | "reset" | "status";
  domains?: string[];
}

export interface HelperResponse {
  ok: boolean;
  active_domains?: string[];
  error?: string;
}

function isRecord(value: JsonValue | undefined): value is JsonRecord {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function isHelperResponse(value: JsonValue): value is JsonRecord & HelperResponse {
  return (
    isRecord(value) &&
    typeof value.ok === "boolean" &&
    (value.active_domains === undefined ||
      (Array.isArray(value.active_domains) && value.active_domains.every(isString))) &&
    (value.error === undefined || typeof value.error === "string")
  );
}

function isString(value: JsonValue): value is string {
  return typeof value === "string";
}

function parseHelperResponse(text: string): HelperResponse {
  const parsed: JsonValue = JSON.parse(text);
  if (isHelperResponse(parsed)) return parsed;
  return { ok: false, error: "invalid helper response shape" };
}

function helperBinaryPath(): string {
  if (app.isPackaged) {
    return path.join(process.resourcesPath, "Kosmos Helper.exe");
  }
  return path.resolve(__dirname, "..", "..", "..", "target", "release", "kepler-focus-helper.exe");
}

// --- Direct spawn (Kepler уже admin) ---------------------------------------

function runHelperDirect(req: HelperRequest): Promise<HelperResponse | "needs_elevation"> {
  return new Promise((resolve) => {
    const bin = helperBinaryPath();
    let stdout = "";
    let stderr = "";

    let child;
    try {
      child = spawn(bin, [], {
        stdio: ["pipe", "pipe", "pipe"],
        windowsHide: true,
      });
    } catch (e: unknown) {
      // SAFETY: Node spawn errors expose the documented errno shape.
      const code = (e as NodeJS.ErrnoException).code;
      // ERROR_ELEVATION_REQUIRED (740) → нужен RunAs flow.
      // SAFETY: Node spawn errors expose an Error-compatible message.
      if (code === "UNKNOWN" || (e as Error).message.includes("740")) {
        resolve("needs_elevation");
        return;
      }
      // SAFETY: The catch value is an Error from the child-process boundary.
      resolve({ ok: false, error: `spawn failed: ${(e as Error).message}` });
      return;
    }

    child.stdout?.on("data", (c: Buffer) => (stdout += c.toString("utf8")));
    child.stderr?.on("data", (c: Buffer) => (stderr += c.toString("utf8")));

    child.on("error", (err: NodeJS.ErrnoException) => {
      // EACCES / EPERM / UNKNOWN при ERROR_ELEVATION_REQUIRED.
      const isElevation =
        err.code === "EACCES" ||
        err.code === "EPERM" ||
        err.code === "UNKNOWN" ||
        (err.message ?? "").includes("740");
      if (isElevation) {
        resolve("needs_elevation");
        return;
      }
      resolve({ ok: false, error: `child error: ${err.message}` });
    });

    child.on("close", (code) => {
      const trimmed = stdout.trim();
      if (!trimmed) {
        resolve({
          ok: false,
          error: `helper exited ${code} with no output. stderr=${stderr.slice(0, 200)}`,
        });
        return;
      }
      try {
        resolve(parseHelperResponse(trimmed));
      } catch {
        resolve({
          ok: false,
          error: `unparseable helper response: ${trimmed.slice(0, 200)}`,
        });
      }
    });

    try {
      child.stdin?.write(JSON.stringify(req));
      child.stdin?.end();
    } catch {
      // Ignore — close handler уже зарезолвит.
    }
  });
}

// --- Elevated spawn через PowerShell Start-Process -Verb RunAs --------------

async function runHelperElevated(req: HelperRequest): Promise<HelperResponse> {
  const bin = helperBinaryPath();
  const id = crypto.randomBytes(6).toString("hex");
  const inputPath = path.join(os.tmpdir(), `kepler-focus-req-${id}.json`);
  const outputPath = path.join(os.tmpdir(), `kepler-focus-resp-${id}.json`);

  try {
    await fsp.writeFile(inputPath, JSON.stringify(req), "utf8");

    // PowerShell escapes:
    //   ' внутри single-quoted string → дублируем
    //   паски с пробелами OK потому что в Argument-List используем single quotes
    const psCommand = [
      "Start-Process",
      "-FilePath",
      `'${bin.replace(/'/g, "''")}'`,
      "-ArgumentList",
      `@('--input','${inputPath.replace(/'/g, "''")}','--output','${outputPath.replace(/'/g, "''")}')`,
      "-Verb",
      "RunAs",
      "-WindowStyle",
      "Hidden",
      "-Wait",
    ].join(" ");

    await new Promise<void>((resolve, reject) => {
      const ps = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", psCommand], {
        windowsHide: true,
      });
      let psStderr = "";
      ps.stderr?.on("data", (c) => (psStderr += c.toString("utf8")));
      ps.on("error", reject);
      ps.on("close", (code) => {
        if (code === 0) resolve();
        else reject(new Error(`powershell exited ${code}: ${psStderr.slice(0, 300)}`));
      });
    });

    const respText = await fsp.readFile(outputPath, "utf8");
    return parseHelperResponse(respText.trim());
  } catch (e) {
    return {
      ok: false,
      // SAFETY: The elevated process boundary reports Error instances.
      error: `elevated run failed: ${(e as Error).message}`,
    };
  } finally {
    // Cleanup temp файлов — best effort.
    await fsp.unlink(inputPath).catch(() => undefined);
    await fsp.unlink(outputPath).catch(() => undefined);
  }
}

async function trySendViaPipe(req: HelperRequest): Promise<HelperResponse | null> {
  try {
    if (!(await pingService())) return null;
    const resp = await sendViaPipe(req);
    if (resp.ok || resp.error) {
      return {
        ok: !!resp.ok,
        active_domains: resp.active_domains,
        error: resp.error,
      };
    }
    return null;
  } catch (e) {
    // SAFETY: The service client rejects with an Error-shaped failure.
    console.warn("[focus-block] service path failed:", (e as Error).message);
    return null;
  }
}

async function runHelper(req: HelperRequest): Promise<HelperResponse> {
  // 1. Service path: pipe всегда даёт zero-UAC если service running.
  const piped = await trySendViaPipe(req);
  if (piped) return piped;

  // Helper fallback: direct spawn (если Kepler сам admin → no UAC).
  const direct = await runHelperDirect(req);
  if (direct !== "needs_elevation") return direct;

  // Elevated helper fallback: PowerShell RunAs (UAC prompt).
  return runHelperElevated(req);
}

// --- Public API -------------------------------------------------------------

export async function applyFocusBlock(args: {
  active: boolean;
  domains: string[];
}): Promise<HelperResponse> {
  const req: HelperRequest = args.active ? { op: "add", domains: args.domains } : { op: "reset" };

  const result = await runHelper(req);

  if (!result.ok) {
    console.warn("[focus-block] helper failed:", result.error);
  } else {
    console.log(
      `[focus-block] applied ${req.op}${req.domains ? ` (${req.domains.length} domains)` : ""}`,
    );
  }

  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      try {
        win.webContents.send("kepler:focus:applied", {
          ok: result.ok,
          error: result.error ?? null,
          activeDomains: result.active_domains ?? [],
        });
      } catch {
        /* ignore */
      }
    }
  }

  return result;
}

export async function getFocusBlockStatus(): Promise<HelperResponse> {
  return runHelper({ op: "status" });
}

export function assertFocusBlockResult(
  result: HelperResponse,
  expectedDomains: readonly string[] | null,
): asserts result is HelperResponse & { active_domains: string[] } {
  if (!result.ok) {
    throw new Error(`focus native enforcement failed: ${result.error ?? "unknown helper error"}`);
  }
  if (!Array.isArray(result.active_domains)) {
    throw new Error("focus native enforcement returned no active domain status");
  }
  if (expectedDomains === null) return;
  const activeDomains = [...result.active_domains].sort();
  const expected = [...expectedDomains].sort();
  if (
    activeDomains.length !== expected.length ||
    expected.some((domain, index) => activeDomains[index] !== domain)
  ) {
    throw new Error("focus native enforcement verification failed");
  }
}
