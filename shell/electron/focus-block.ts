// Focus block applier — спавнит `kepler-focus-helper.exe` для модификации
// hosts файла когда юзер активирует/деактивирует focus blocklist.
//
// Helper bin требует admin elevation (`requireAdministrator` manifest).
// Если Kepler запущен НЕ от админа — Windows показывает UAC prompt при
// первом спавне. Юзер должен одобрить (раз за сессию OS обычно).
//
// Wire-up: extension-host.ts hooks `kepler:extension:ark:request` →
// после успешного `focus.set_active_state` вызывает `applyFocusBlock(...)`.

import { spawn } from "node:child_process";
import path from "node:path";
import { app, BrowserWindow } from "electron";
import { fileURLToPath } from "node:url";

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
  // Production install: helper.exe лежит рядом с resources/.
  // app.getAppPath() → .asar или dev source.
  // Используем `process.resourcesPath` в production, dev path → target/release/.
  if (app.isPackaged) {
    return path.join(process.resourcesPath, "kepler-focus-helper.exe");
  }
  // Dev: relative от <repo>/shell/electron/main.ts → ../../target/release/.
  return path.resolve(__dirname, "..", "..", "target", "release", "kepler-focus-helper.exe");
}

function runHelper(req: HelperRequest): Promise<HelperResponse> {
  return new Promise((resolve) => {
    const bin = helperBinaryPath();
    let stdout = "";
    let stderr = "";

    let child;
    try {
      child = spawn(bin, [], { stdio: ["pipe", "pipe", "pipe"], windowsHide: true });
    } catch (e) {
      resolve({ ok: false, error: `spawn failed: ${(e as Error).message}` });
      return;
    }

    child.stdout?.on("data", (chunk: Buffer) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr?.on("data", (chunk: Buffer) => {
      stderr += chunk.toString("utf8");
    });

    child.on("error", (err) => {
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
    } catch (e) {
      resolve({ ok: false, error: `stdin write failed: ${(e as Error).message}` });
    }
  });
}

// --- Public API -------------------------------------------------------------

/**
 * Применить блокировку доменов. Вызывается из extension-host после
 * `focus.set_active_state` приходящего от Horologion / Settings UI.
 *
 * - `active=true, domains=[...]` → helper.exe op=add, domains добавлены в hosts.
 * - `active=false` → helper.exe op=reset, kepler-managed section в hosts очищена.
 *
 * Результат log'ируется и broadcast'ится всем BrowserWindow'ам через
 * `kepler:focus:applied` event (для UI feedback в Settings).
 */
export async function applyFocusBlock(args: {
  active: boolean;
  domains: string[];
}): Promise<HelperResponse> {
  const req: HelperRequest = args.active
    ? { op: "add", domains: args.domains }
    : { op: "reset" };

  const result = await runHelper(req);

  if (!result.ok) {
    console.warn("[focus-block] helper failed:", result.error);
  } else {
    console.log(
      `[focus-block] applied ${req.op}${
        req.domains ? ` (${req.domains.length} domains)` : ""
      }`,
    );
  }

  // Broadcast результат всем окнам — UI может показать status / error.
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

/** Reset на app shutdown / uninstall — гарантируем что hosts не остаётся
    модифицированным после Kepler exit. */
export async function resetFocusBlockOnShutdown(): Promise<void> {
  try {
    await runHelper({ op: "reset" });
  } catch (e) {
    console.warn("[focus-block] shutdown reset failed:", e);
  }
}
