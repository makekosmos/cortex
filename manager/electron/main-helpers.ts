import { app } from "electron";
import path from "node:path";
import fs from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { resolvePackagedHostExecutable, resolvePackagedRuntimeExecutable } from "./host-resolution";
import { resolveInstance } from "../../desktop/electron/instance";
import { keplerDataDir } from "../../desktop/electron/data-dir";
import { resolveDesktopUpdateBridge } from "./desktop-update-bridge";
import type { DevPackage } from "./dev-packages";
import { isNumber, isObject, isString, type Input } from "./manager-contract";
function openHostedPackage(id: string, development?: DevPackage) {
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
    const child = spawn(executable, hostArgs(id, development), {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
      env: development ? { ...process.env, KOSMOS_DEV_MODE: "1" } : process.env,
    });
    child.unref();
    return true;
  }
  if (devMain && fs.existsSync(devMain)) {
    const child = spawn(process.execPath, [devMain, ...hostArgs(id, development)], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
      env: development ? { ...process.env, KOSMOS_DEV_MODE: "1" } : process.env,
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

function hostArgs(id: string, development?: DevPackage): string[] {
  return development ? [`--open-app=${id}`, `--dev-url=${development.url}`] : [`--open-app=${id}`];
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

const bounded = (value: Input, max: number): value is string =>
  isString(value) && value.trim().length > 0 && value.length <= max;
const validation = (condition: boolean, message = "Проверьте введённые данные."): string | null =>
  condition ? null : message;
const validIntegrationId = (value: Input): value is string =>
  isString(value) && /^[a-z0-9][a-z0-9._-]{0,127}$/.test(value);
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
    return isObject(value) && isString(value.kind) ? value : null;
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
const validIntegrationInput = (value: Input): value is { provider: string } =>
  isObject(value) && validIntegrationId(value.provider);
function normalizeIntegrationSnapshot(value: Input) {
  const raw = isObject(value) ? value : {};
  const providers = Array.isArray(raw.providers) ? raw.providers : [];
  return {
    providers: providers.flatMap((entry: Input) => {
      if (!isObject(entry) || !validIntegrationId(entry.id)) return [];
      const settings = isObject(entry.settings) ? entry.settings : {};
      const settingSchema = Array.isArray(entry.settingSchema)
        ? entry.settingSchema.flatMap((setting: Input) =>
            isObject(setting) &&
            validIntegrationId(setting.key) &&
            isString(setting.label) &&
            (setting.kind === "text" || setting.kind === "secret")
              ? [
                  {
                    key: setting.key,
                    label: setting.label.slice(0, 128),
                    kind: setting.kind,
                    description: isString(setting.description)
                      ? setting.description.slice(0, 512)
                      : undefined,
                    required: setting.required === true,
                  },
                ]
              : [],
          )
        : [];
      const settingValues = isObject(entry.settingValues)
        ? Object.fromEntries(
            Object.entries(entry.settingValues).filter(
              ([key, value]) => validIntegrationId(key) && isString(value) && value.length <= 4096,
            ),
          )
        : {};
      return [
        {
          id: entry.id,
          label: isString(entry.label) ? entry.label.slice(0, 64) : entry.id,
          credentialLabel: isString(entry.credentialLabel)
            ? entry.credentialLabel.slice(0, 64)
            : "Ключ",
          credentialUrl: isString(entry.credentialUrl) ? entry.credentialUrl.slice(0, 256) : "",
          hasCredential: entry.hasCredential === true,
          iconPath:
            isString(entry.iconPath) &&
            entry.iconPath.length <= 2048 &&
            (/^[A-Za-z]:[\\/]/.test(entry.iconPath) || entry.iconPath.startsWith("/"))
              ? entry.iconPath
              : undefined,
          packageManaged: entry.packageManaged === true,
          enabled: entry.enabled === true,
          iconKey: validIntegrationId(entry.iconKey) ? entry.iconKey : entry.id,
          authMode:
            entry.authMode === "browser_login" || entry.authMode === "none"
              ? entry.authMode
              : "credential",
          credentialInputType: entry.credentialInputType === "text" ? "text" : "password",
          loginCapability: validIntegrationId(entry.loginCapability)
            ? entry.loginCapability
            : undefined,
          settingSchema,
          settingValues,
          settings: {
            intervalMinutes:
              Number.isInteger(settings.intervalMinutes) &&
              Number(settings.intervalMinutes) >= 0 &&
              Number(settings.intervalMinutes) <= 7 * 24 * 60
                ? Number(settings.intervalMinutes)
                : 0,
            syncOnStartup: settings.syncOnStartup === true,
            lastAttemptAt: isString(settings.lastAttemptAt)
              ? settings.lastAttemptAt.slice(0, 64)
              : null,
            lastSuccessAt: isString(settings.lastSuccessAt)
              ? settings.lastSuccessAt.slice(0, 64)
              : null,
            lastError: isString(settings.lastError) ? settings.lastError.slice(0, 256) : null,
            importedCount:
              Number.isInteger(settings.importedCount) && Number(settings.importedCount) >= 0
                ? Number(settings.importedCount)
                : 0,
          },
        },
      ];
    }),
    bodyWeightKg: isNumber(raw.bodyWeightKg) ? raw.bodyWeightKg : null,
  };
}

export {
  openHostedPackage,
  startPackagedRuntime,
  bounded,
  isObject,
  validation,
  AUTOSTART_ARGS,
  readDesktopUpdateState,
  requestDesktopUpdate,
  validIntegrationInput,
  normalizeIntegrationSnapshot,
};
