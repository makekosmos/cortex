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
// Wire-up: extension-host.ts hooks `kepler:extension:ark:request` →
// после успешного `focus.set_active_state` вызывает `applyFocusBlock(...)`.

import { spawn } from "node:child_process";
import { promises as fsp } from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";
import { app, BrowserWindow } from "electron";
import { fileURLToPath } from "node:url";
import { getServiceStatus, pingService, runServiceCliElevated, sendViaPipe } from "./focus-service";
import {
  isFocusServiceAutoInstallDeclined,
  setFocusServiceAutoInstallDeclined,
} from "./settings-window";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

interface HelperRequest {
  op: "add" | "remove" | "reset" | "status";
  domains?: string[];
}

interface HelperResponse {
  ok: boolean;
  active_domains?: string[];
  error?: string;
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
      child = spawn(bin, [], { stdio: ["pipe", "pipe", "pipe"], windowsHide: true });
    } catch (e: unknown) {
      const code = (e as NodeJS.ErrnoException).code;
      // ERROR_ELEVATION_REQUIRED (740) → нужен RunAs flow.
      if (code === "UNKNOWN" || (e as Error).message.includes("740")) {
        resolve("needs_elevation");
        return;
      }
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
        const parsed = JSON.parse(trimmed) as HelperResponse;
        resolve(parsed);
      } catch {
        resolve({ ok: false, error: `unparseable helper response: ${trimmed.slice(0, 200)}` });
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
    return JSON.parse(respText.trim()) as HelperResponse;
  } catch (e) {
    return {
      ok: false,
      error: `elevated run failed: ${(e as Error).message}`,
    };
  } finally {
    // Cleanup temp файлов — best effort.
    await fsp.unlink(inputPath).catch(() => undefined);
    await fsp.unlink(outputPath).catch(() => undefined);
  }
}

// Session-scoped: пометка что мы уже пытались auto-install service'а в эту
// сессию и юзер либо отменил UAC, либо install упал. Без этой пометки каждое
// включение блокировки будет триггерить UAC prompt установки.
let autoInstallAttemptedThisSession = false;

async function tryAutoInstallService(): Promise<boolean> {
  if (autoInstallAttemptedThisSession) return false;
  if (isFocusServiceAutoInstallDeclined()) return false;
  autoInstallAttemptedThisSession = true;

  console.log("[focus-block] auto-install kepler-focus-svc (one-time UAC prompt)");
  const installResult = await runServiceCliElevated("install");
  if (!installResult.ok) {
    // User cancelled UAC, или install реально упал. Persistим decline чтобы
    // повторно не спрашивать в следующих сессиях — юзер может включить через
    // Settings UI вручную.
    console.warn("[focus-block] auto-install failed/declined:", installResult.error);
    setFocusServiceAutoInstallDeclined(true);
    return false;
  }

  // Сбрасываем декланд флаг если он был — install прошёл успешно.
  setFocusServiceAutoInstallDeclined(false);

  // Notify renderer'ы — Settings → Focus покажет «daemon установлен».
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      try {
        win.webContents.send("kepler:focus-service:status-changed");
      } catch {
        /* ignore */
      }
    }
  }

  // Install handler уже стартанул service (см. cli.rs install). Просто
  // verify status + ping. Poll до 3s — service handshake'ит pipe чуть-чуть.
  for (let i = 0; i < 30; i++) {
    const status = await getServiceStatus();
    if (status.installed && status.running) {
      if (await pingService()) return true;
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  console.warn("[focus-block] auto-install ok but pipe не отвечает за 3s");
  return false;
}

async function trySendViaPipe(req: HelperRequest): Promise<HelperResponse | null> {
  try {
    if (!(await pingService())) return null;
    const resp = await sendViaPipe(req);
    if (resp.ok || resp.error) {
      return { ok: !!resp.ok, active_domains: resp.active_domains, error: resp.error };
    }
    return null;
  } catch (e) {
    console.warn("[focus-block] service path failed:", (e as Error).message);
    return null;
  }
}

async function runHelper(req: HelperRequest): Promise<HelperResponse> {
  // 1. Service path: pipe всегда даёт zero-UAC если service running.
  const piped = await trySendViaPipe(req);
  if (piped) return piped;

  // 2. Auto-install service: один UAC сейчас → zero UAC потом.
  //    Только если юзер не отклонил это раньше. Промазав, идём дальше на
  //    helper-фоллбэк (UAC per-call, как было раньше).
  const installed = await tryAutoInstallService();
  if (installed) {
    const retry = await trySendViaPipe(req);
    if (retry) return retry;
  }

  // 3. Helper fallback: direct spawn (если Kepler сам admin → no UAC).
  const direct = await runHelperDirect(req);
  if (direct !== "needs_elevation") return direct;

  // 4. Elevated helper fallback: PowerShell RunAs (UAC prompt).
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
