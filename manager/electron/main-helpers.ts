import { app, BrowserWindow, type WebContents } from "electron";
import path from "node:path";
import fs from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { encodeLeetCodeCredential } from "./leetcode-login-flow";
import {
  resolvePackagedHostExecutable,
  resolvePackagedRuntimeExecutable,
} from "./host-resolution";
import { resolveInstance } from "../../desktop/electron/instance";
import { keplerDataDir } from "../../desktop/electron/data-dir";
import { resolveDesktopUpdateBridge } from "./desktop-update-bridge";
function openHostedPackage(id: string) {
  if (
    !/^[a-z0-9][a-z0-9._-]{0,127}$/.test(id) ||
    process.env.KOSMOS_HEADLESS === "1" ||
    process.env.KOSMOS_TEST_MODE === "1"
  )
    return false;
  const configured = process.env.KOSMOS_HOST_EXECUTABLE?.trim();
  const executable = configured && path.resolve(configured);
  const devMain =
    process.env.KOSMOS_HOST_MAIN?.trim() && path.resolve(process.env.KOSMOS_HOST_MAIN);
  const packaged = resolvePackagedHostExecutable(process.resourcesPath);
  if (executable && fs.existsSync(executable)) {
    const child = spawn(executable, [`--open-app=${id}`], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
    });
    child.unref();
    return true;
  }
  if (devMain && fs.existsSync(devMain)) {
    const child = spawn(process.execPath, [devMain, `--open-app=${id}`], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
    });
    child.unref();
    return true;
  }
  if (app.isPackaged && fs.existsSync(packaged)) {
    const child = spawn(packaged, [`--open-app=${id}`], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
    });
    child.unref();
    return true;
  }
  return false;
}

function startPackagedRuntime(): boolean {
  const executable = resolvePackagedRuntimeExecutable(process.resourcesPath);
  if (!app.isPackaged || process.platform !== "win32" || !fs.existsSync(executable)) return false;
  const child = spawn(executable, ["--start"], {
    detached: true,
    stdio: "ignore",
    windowsHide: true,
  });
  child.once("error", () => undefined);
  child.unref();
  return true;
}

const bounded = (value: unknown, max: number): value is string =>
  typeof value === "string" && value.trim().length > 0 && value.length <= max;
const isObject = (value: unknown): value is Record<string, unknown> =>
  Boolean(value) && typeof value === "object" && !Array.isArray(value);
const validation = (condition: boolean, message = "Проверьте введённые данные."): string | null =>
  condition ? null : message;
const integrationProviders = ["hevy", "toggl", "leetcode", "codewars"];
const integrationIntervals = [0, 15, 60, 360, 1440];
const LEETCODE_PARTITION = "persist:kosmos-manager-leetcode";
const AUTOSTART_ARGS = ["--autostart"];
const desktopUpdateBridge = () =>
  resolveDesktopUpdateBridge({
    env: process.env,
    instance: resolveInstance(),
    dataDir: keplerDataDir(),
    resourcesPath: process.resourcesPath,
    platform: process.platform,
    isPackaged: app.isPackaged,
    exists: fs.existsSync,
  });
function readDesktopUpdateState() {
  try {
    const bridge = desktopUpdateBridge();
    if (!bridge) return null;
    const value = JSON.parse(fs.readFileSync(bridge.stateFile, "utf8"));
    return value && typeof value.kind === "string" ? value : null;
  } catch {
    return null;
  }
}
function requestDesktopUpdate(flag: "--kosmos-update-check" | "--kosmos-update-install") {
  const bridge = desktopUpdateBridge();
  if (!bridge) return false;
  spawnSync(bridge.executable, [flag], {
    windowsHide: process.platform === "win32",
    stdio: "ignore",
    timeout: 5_000,
  });
  return true;
}
const validIntegrationInput = (value: unknown): value is { provider: string } =>
  isObject(value) &&
  typeof value.provider === "string" &&
  integrationProviders.includes(value.provider);
function normalizeIntegrationSnapshot(value: unknown) {
  const raw = isObject(value) ? value : {};
  const providers = Array.isArray(raw.providers) ? raw.providers : [];
  return {
    providers: providers.flatMap((entry) => {
      if (
        !isObject(entry) ||
        typeof entry.id !== "string" ||
        !integrationProviders.includes(entry.id)
      )
        return [];
      const settings = isObject(entry.settings) ? entry.settings : {};
      return [
        {
          id: entry.id,
          label: typeof entry.label === "string" ? entry.label.slice(0, 64) : entry.id,
          credentialLabel:
            typeof entry.credentialLabel === "string" ? entry.credentialLabel.slice(0, 64) : "Ключ",
          credentialUrl:
            typeof entry.credentialUrl === "string" ? entry.credentialUrl.slice(0, 256) : "",
          hasCredential: entry.hasCredential === true,
          settings: {
            intervalMinutes:
              Number.isInteger(settings.intervalMinutes) &&
              integrationIntervals.includes(Number(settings.intervalMinutes))
                ? Number(settings.intervalMinutes)
                : 0,
            syncOnStartup: settings.syncOnStartup === true,
            lastAttemptAt:
              typeof settings.lastAttemptAt === "string"
                ? settings.lastAttemptAt.slice(0, 64)
                : null,
            lastSuccessAt:
              typeof settings.lastSuccessAt === "string"
                ? settings.lastSuccessAt.slice(0, 64)
                : null,
            lastError:
              typeof settings.lastError === "string" ? settings.lastError.slice(0, 256) : null,
            importedCount:
              Number.isInteger(settings.importedCount) && Number(settings.importedCount) >= 0
                ? Number(settings.importedCount)
                : 0,
          },
        },
      ];
    }),
    bodyWeightKg: typeof raw.bodyWeightKg === "number" ? raw.bodyWeightKg : null,
  };
}

async function waitForLeetCodeCredential(win: BrowserWindow): Promise<string> {
  return new Promise((resolve, reject) => {
    let settled = false;
    let timeout: ReturnType<typeof setTimeout> | null = null;
    const cookies = win.webContents.session.cookies;
    const finish = (reason?: "cancelled" | "timeout", credential?: string) => {
      if (settled) return;
      settled = true;
      if (timeout) clearTimeout(timeout);
      cookies.removeListener("changed", onCookieChanged);
      win.removeListener("closed", onClosed);
      if (reason) reject(new Error(reason));
      else resolve(credential!);
    };
    const onClosed = () => finish("cancelled");
    const check = async () => {
      if (win.isDestroyed()) return;
      const [sessionCookie, csrfCookie] = await Promise.all([
        cookies.get({ url: "https://leetcode.com/", name: "LEETCODE_SESSION" }),
        cookies.get({ url: "https://leetcode.com/", name: "csrftoken" }),
      ]);
      const credential = encodeLeetCodeCredential(sessionCookie[0]?.value, csrfCookie[0]?.value);
      if (credential) finish(undefined, credential);
    };
    const onCookieChanged = () => void check().catch(() => undefined);
    timeout = setTimeout(() => finish("timeout"), 5 * 60_000);
    cookies.on("changed", onCookieChanged);
    win.once("closed", onClosed);
    void check().catch(() => undefined);
  });
}

function createLeetCodeLoginWindow(sender: WebContents): BrowserWindow {
  const parent = BrowserWindow.fromWebContents(sender) ?? undefined;
  return new BrowserWindow({
    parent,
    modal: Boolean(parent),
    width: 1080,
    height: 760,
    title: "Вход в LeetCode",
    autoHideMenuBar: true,
    webPreferences: {

      partition: LEETCODE_PARTITION,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
}
export { openHostedPackage, startPackagedRuntime, bounded, isObject, validation, integrationProviders, integrationIntervals, LEETCODE_PARTITION, AUTOSTART_ARGS, readDesktopUpdateState, requestDesktopUpdate, validIntegrationInput, normalizeIntegrationSnapshot, waitForLeetCodeCredential, createLeetCodeLoginWindow };
