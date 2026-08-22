import {
  app,
  BrowserWindow,
  dialog,
  ipcMain,
  Menu,
  session,
  shell,
  type WebContents,
  type OpenDialogOptions,
} from "electron";
import path from "node:path";
import fs from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { rpc, status, subscribeDictationEvents, waitForEngineReady } from "./engine-client";
import { managerOperations as op, type ManagerResult, type SyncSnapshot } from "../src/manager-api";
import {
  normalizeConnectivity,
  normalizeFocusActiveState,
  normalizeFocusBlocklists,
  normalizeFocusServiceStatus,
  isActiveFocusBlocklist,
  isValidFocusActiveState,
  normalizeDictationConfig,
  normalizeDictationEvent,
  normalizeDictationLocalModels,
  normalizeDictationPending,
  normalizeDictationStats,
  normalizePairingCode,
  normalizeSyncSnapshot,
  normalizeFileIndexDiagnostics,
  normalizeFileIndexSettings,
  normalizeEngineSettings,
  normalizePackageSnapshot,
  normalizePackageTrustStatus,
  validPackageId,
  validPackageVersion,
  validateFileIndexPath,
  validateFileIndexSettingsPatch,
  validateUsageTrackerSettingsPatch,
  toDeviceParams,
  toPairingParams,
  validateDictationConfigPatch,
  validPairingCode,
  validDictationId,
  validFocusId,
} from "./manager-contract";
import {
  getServiceStatus,
  pingService,
  runServiceCliElevated,
} from "../../shared/focus-service-client";
import {
  clearCrashReports,
  ensureDiagnosticsDir,
  listCrashReports,
  resolveManagerDataDir,
} from "./diagnostics-files";
import { encodeLeetCodeCredential, runLeetCodeLogin } from "./leetcode-login-flow";
import { resolvePackagedHostExecutable } from "./host-resolution";
import { resolveInstance } from "../../desktop/electron/instance";
import { keplerDataDir } from "../../desktop/electron/data-dir";
import { resolveDesktopUpdateBridge } from "./desktop-update-bridge";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
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
let dictationSubscriber: WebContents | null = null;
let stopDictationEvents: (() => void) | null = null;

function addDictationSubscriber(sender: WebContents) {
  dictationSubscriber = sender;
  sender.once("destroyed", () => {
    if (dictationSubscriber === sender) dictationSubscriber = null;
  });
  if (stopDictationEvents) return;
  stopDictationEvents = subscribeDictationEvents((raw) => {
    const event = normalizeDictationEvent(raw);
    if (!event) return;
    if (dictationSubscriber && !dictationSubscriber.isDestroyed())
      dictationSubscriber.send("manager:dictation-event", event);
  });
}

function register(
  name: string,
  operation: string,
  validate?: (value: unknown) => string | null,
  transform?: (value: unknown) => Record<string, unknown>,
) {
  ipcMain.handle(name, async (_event, value) => {
    const error = validate?.(value);
    if (error) return { ok: false, code: "validation", message: error };
    return rpc(
      operation,
      transform ? transform(value) : (value as Record<string, unknown> | undefined),
    );
  });
}

function registerAll() {
  ipcMain.handle("manager.getAppVersion", () => ({
    ok: true,
    data: app.getVersion(),
  }));
  ipcMain.handle("manager.getDesktopUpdateState", () => {
    const state = readDesktopUpdateState();
    return state
      ? { ok: true, data: state }
      : {
          ok: false,
          code: "engine",
          message: "Kosmos Desktop недоступен для проверки обновлений.",
        };
  });
  ipcMain.handle("manager.checkDesktopUpdates", () => {
    if (!requestDesktopUpdate("--kosmos-update-check"))
      return {
        ok: false,
        code: "engine",
        message: "Kosmos Desktop недоступен для проверки обновлений.",
      };
    return { ok: true, data: readDesktopUpdateState() ?? { kind: "checking" } };
  });
  ipcMain.handle("manager.installDesktopUpdate", () => {
    const state = readDesktopUpdateState();
    if (state?.kind !== "downloaded")
      return {
        ok: false,
        code: "validation",
        message: "Скачанное обновление Desktop пока не готово.",
      };
    return requestDesktopUpdate("--kosmos-update-install")
      ? { ok: true, data: { started: true } }
      : { ok: false, code: "engine", message: "Kosmos Desktop недоступен для перезапуска." };
  });
  ipcMain.handle("manager.getHealth", async () => status("/v1/health"));
  ipcMain.handle("manager.getInfo", async () => status("/v1/info"));
  register("manager.getDataSummary", op.getDataSummary);
  register("manager.listObjectTypes", op.listObjectTypes);
  register("manager.listObjects", op.listObjects, (value) =>
    validation(
      value === undefined ||
        (typeof value === "object" &&
          value !== null &&
          (!("type_id" in value) ||
            (value as { type_id?: unknown }).type_id === undefined ||
            bounded((value as { type_id?: unknown }).type_id, 128)) &&
          (!("cursor" in value) ||
            (value as { cursor?: unknown }).cursor === undefined ||
            (typeof (value as { cursor?: unknown }).cursor === "string" &&
              (value as { cursor: string }).cursor.length <= 20)) &&
          (!("limit" in value) ||
            (Number.isInteger((value as { limit?: unknown }).limit) &&
              Number((value as { limit?: unknown }).limit) >= 1 &&
              Number((value as { limit?: unknown }).limit) <= 200))),
    ),
  );
  register("manager.searchObjects", op.searchObjects, (value) =>
    validation(typeof value === "object" && bounded((value as { query?: unknown }).query, 256)),
  );
  ipcMain.handle("manager.getSyncSnapshot", async (): Promise<ManagerResult<SyncSnapshot>> => {
    const result = await rpc(op.getSyncSnapshot);
    return result.ok ? { ok: true, data: normalizeSyncSnapshot(result.data) } : result;
  });
  ipcMain.handle("manager.getPairingCode", async () => {
    const result = await rpc(op.getPairingCode);
    if (!result.ok) return result;
    const rawCode =
      typeof result.data === "string" ? result.data : (result.data as { code?: unknown })?.code;
    if (result.data == null) return { ok: true, data: null };
    const code = normalizePairingCode(rawCode);
    return code
      ? { ok: true, data: { code } }
      : {
          ok: false,
          code: "engine",
          message: "Движок не вернул код подключения.",
        };
  });
  register(
    "manager.connectWithPairingCode",
    op.connectWithPairingCode,
    (value) =>
      validation(typeof value === "object" && validPairingCode((value as { code?: unknown }).code)),
    (value) => toPairingParams((value as { code: string }).code),
  );
  ipcMain.handle("manager.getIntegrations", async () => {
    const result = await rpc(op.getIntegrations);
    return result.ok ? { ok: true, data: normalizeIntegrationSnapshot(result.data) } : result;
  });
  ipcMain.handle("manager.loginLeetCode", async (event, value) => {
    if (value !== undefined)
      return {
        ok: false,
        code: "validation",
        message: "Недопустимые параметры входа.",
      };
    if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1")
      return {
        ok: false,
        code: "engine_unavailable",
        message: "Вход через LeetCode недоступен в этом режиме.",
      };
    if (BrowserWindow.fromWebContents(event.sender) !== managerWindow)
      return {
        ok: false,
        code: "validation",
        message: "Недопустимый источник входа.",
      };
    try {
      return await runLeetCodeLogin({
        clearCookies: () =>
          session.fromPartition(LEETCODE_PARTITION).clearStorageData({ storages: ["cookies"] }),
        createWindow: () => createLeetCodeLoginWindow(event.sender),
        loadLogin: (win) => win.loadURL("https://leetcode.com/accounts/login/"),
        waitForCredential: waitForLeetCodeCredential,
        closeWindow: (win) => {
          if (!win.isDestroyed()) win.close();
        },
        persistCredential: async (credential) => {
          const result = await rpc(op.setIntegrationCredential, {
            provider: "leetcode",
            credential,
          });
          return result.ok ? { ok: true, data: normalizeIntegrationSnapshot(result.data) } : result;
        },
      });
    } catch (cause) {
      if (cause instanceof Error && cause.message === "cancelled")
        return {
          ok: false,
          code: "cancelled",
          message: "Вход в LeetCode отменён.",
        };
      if (cause instanceof Error && cause.message === "timeout")
        return {
          ok: false,
          code: "cancelled",
          message: "Время входа в LeetCode истекло.",
        };
      return {
        ok: false,
        code: "engine",
        message: "Не удалось завершить вход в LeetCode.",
      };
    }
  });
  ipcMain.handle("manager.updateIntegrationSettings", async (_event, value) => {
    if (!validIntegrationInput(value))
      return {
        ok: false,
        code: "validation",
        message: "Недопустимая интеграция.",
      };
    const input = value as Record<string, unknown>;
    if (
      input.intervalMinutes !== undefined &&
      (!Number.isInteger(input.intervalMinutes) ||
        !integrationIntervals.includes(Number(input.intervalMinutes)))
    )
      return {
        ok: false,
        code: "validation",
        message: "Недопустимая частота синхронизации.",
      };
    if (input.syncOnStartup !== undefined && typeof input.syncOnStartup !== "boolean")
      return {
        ok: false,
        code: "validation",
        message: "Недопустимое значение запуска.",
      };
    const result = await rpc(op.updateIntegrationSettings, input);
    return result.ok ? { ok: true, data: normalizeIntegrationSnapshot(result.data) } : result;
  });
  ipcMain.handle("manager.setIntegrationCredential", async (_event, value) => {
    if (
      !validIntegrationInput(value) ||
      typeof (value as Record<string, unknown>).credential !== "string" ||
      !(value as Record<string, unknown>).credential ||
      String((value as Record<string, unknown>).credential).length > 2048
    )
      return {
        ok: false,
        code: "validation",
        message: "Введите корректные данные подключения.",
      };
    const input = value as Record<string, unknown>;
    const result = await rpc(op.setIntegrationCredential, {
      provider: input.provider,
      credential: String(input.credential).trim(),
    });
    return result.ok ? { ok: true, data: normalizeIntegrationSnapshot(result.data) } : result;
  });
  ipcMain.handle("manager.clearIntegrationCredential", async (_event, value) => {
    if (!validIntegrationInput(value))
      return {
        ok: false,
        code: "validation",
        message: "Недопустимая интеграция.",
      };
    const result = await rpc(op.clearIntegrationCredential, {
      provider: value.provider,
    });
    return result.ok ? { ok: true, data: normalizeIntegrationSnapshot(result.data) } : result;
  });
  ipcMain.handle("manager.syncIntegrationNow", async (_event, value) => {
    if (!validIntegrationInput(value))
      return {
        ok: false,
        code: "validation",
        message: "Недопустимая интеграция.",
      };
    const synced = await rpc(op.syncIntegrationNow, {
      provider: value.provider,
    });
    if (!synced.ok) return synced;
    const refreshed = await rpc(op.getIntegrations);
    return refreshed.ok
      ? { ok: true, data: normalizeIntegrationSnapshot(refreshed.data) }
      : refreshed;
  });

  ipcMain.handle("manager.getDictationConfig", async () => {
    const result = await rpc(op.getDictationConfig);
    return result.ok ? { ok: true, data: normalizeDictationConfig(result.data) } : result;
  });
  ipcMain.handle("manager.subscribeDictationEvents", async (event) => {
    addDictationSubscriber(event.sender);
    return { ok: true, data: { subscribed: true } };
  });
  ipcMain.handle("manager.unsubscribeDictationEvents", async (event) => {
    if (dictationSubscriber === event.sender) {
      dictationSubscriber = null;
      stopDictationEvents?.();
      stopDictationEvents = null;
    }
    return { ok: true, data: { subscribed: false } };
  });
  ipcMain.handle("manager.updateDictationConfig", async (_event, value) => {
    const input = validateDictationConfigPatch(value);
    if (!input)
      return {
        ok: false,
        code: "validation",
        message: "Недопустимая конфигурация диктовки.",
      };
    const result = await rpc(op.updateDictationConfig, input);
    if (!result.ok) return result;
    const readback = await rpc(op.getDictationConfig);
    return readback.ok ? { ok: true, data: normalizeDictationConfig(readback.data) } : readback;
  });
  ipcMain.handle("manager.getDictationStats", async () => {
    const result = await rpc(op.getDictationStats);
    return result.ok ? { ok: true, data: normalizeDictationStats(result.data) } : result;
  });
  for (const [channel, operation, data] of [
    ["manager.beginDictationHotkeyCapture", op.beginDictationHotkeyCapture, { started: true }],
    ["manager.endDictationHotkeyCapture", op.endDictationHotkeyCapture, { ended: true }],
  ] as const) {
    ipcMain.handle(channel, async () => {
      const result = await rpc(operation);
      return result.ok ? { ok: true, data } : result;
    });
  }
  ipcMain.handle("manager.testDictationConnectivity", async () => {
    const result = await rpc(op.testDictationConnectivity);
    return result.ok ? { ok: true, data: normalizeConnectivity(result.data) } : result;
  });
  ipcMain.handle("manager.clearDictationApiKey", async () => {
    const result = await rpc(op.clearDictationApiKey);
    return result.ok ? { ok: true, data: { cleared: true } } : result;
  });
  for (const [channel, operation, field] of [
    ["manager.retryAllDictation", op.retryAllDictation, "started"],
    ["manager.discardAllDictation", op.discardAllDictation, "discarded"],
  ] as const) {
    ipcMain.handle(channel, async () => {
      const result = await rpc(operation);
      const raw =
        result.ok && result.data && typeof result.data === "object"
          ? (result.data as Record<string, unknown>)
          : {};
      return result.ok
        ? {
            ok: true,
            data: {
              [field]:
                Number.isInteger(raw[field]) && Number(raw[field]) >= 0 ? Number(raw[field]) : 0,
            },
          }
        : result;
    });
  }
  for (const [channel, operation] of [
    ["manager.verifyDictationApiKey", op.verifyDictationApiKey],
    ["manager.setDictationApiKey", op.setDictationApiKey],
  ] as const) {
    ipcMain.handle(channel, async (_event, value) => {
      const input = value && typeof value === "object" ? (value as Record<string, unknown>) : {};
      const key = input.key;
      if (Object.keys(input).length !== 1 || !bounded(key, 512))
        return { ok: false, code: "validation", message: "Введите ключ." };
      const result = await rpc(operation, { key });
      const raw =
        result.ok && result.data && typeof result.data === "object"
          ? (result.data as Record<string, unknown>)
          : {};
      return result.ok
        ? {
            ok: true,
            data:
              operation === op.verifyDictationApiKey
                ? {
                    valid: raw.ok === true,
                    message:
                      typeof raw.reason === "string"
                        ? raw.reason.slice(0, 256)
                        : "Проверка завершена.",
                  }
                : { saved: true },
          }
        : result;
    });
  }
  const dictationInput = (channel: string, operation: string, field: "modelId" | "uuid") =>
    ipcMain.handle(channel, async (_event, value) => {
      const input = value && typeof value === "object" ? (value as Record<string, unknown>) : {};
      const v = input[field];
      if (!validDictationId(v))
        return {
          ok: false,
          code: "validation",
          message: "Недопустимое значение.",
        };
      const params: Record<string, unknown> = { [field]: v };
      if (field === "modelId") {
        if (
          Object.keys(input).some((key) => key !== "modelId" && key !== "select") ||
          ("select" in input && typeof input.select !== "boolean")
        )
          return {
            ok: false,
            code: "validation",
            message: "Недопустимые параметры модели.",
          };
        params.select = input.select === true;
      } else if (Object.keys(input).some((key) => key !== "uuid"))
        return {
          ok: false,
          code: "validation",
          message: "Недопустимые параметры очереди.",
        };
      const result = await rpc(operation, params);
      if (!result.ok) return result;
      if (operation === op.downloadDictationLocalModel)
        return { ok: true, data: { started: true, modelId: v } };
      if (operation === op.useDictationLocalModel || operation === op.deleteDictationLocalModel) {
        const readback = await rpc(op.getDictationConfig);
        return readback.ok ? { ok: true, data: normalizeDictationConfig(readback.data) } : readback;
      }
      return {
        ok: true,
        data: operation === op.retryDictation ? { started: true } : { discarded: true },
      };
    });
  dictationInput("manager.downloadDictationLocalModel", op.downloadDictationLocalModel, "modelId");
  dictationInput("manager.useDictationLocalModel", op.useDictationLocalModel, "modelId");
  dictationInput("manager.deleteDictationLocalModel", op.deleteDictationLocalModel, "modelId");
  dictationInput("manager.retryDictation", op.retryDictation, "uuid");
  dictationInput("manager.discardDictation", op.discardDictation, "uuid");
  ipcMain.handle("manager.listDictationLocalModels", async () => {
    const result = await rpc(op.listDictationLocalModels);
    return result.ok ? { ok: true, data: normalizeDictationLocalModels(result.data) } : result;
  });
  ipcMain.handle("manager.listDictationPending", async () => {
    const result = await rpc(op.listDictationPending);
    return result.ok ? { ok: true, data: normalizeDictationPending(result.data) } : result;
  });
  ipcMain.handle("manager.getFocusBlocklists", async () => {
    const result = await rpc(op.getFocusBlocklists);
    return result.ok ? { ok: true, data: normalizeFocusBlocklists(result.data) } : result;
  });
  ipcMain.handle("manager.getFocusActiveState", async () => {
    const result = await rpc(op.getFocusActiveState);
    return result.ok ? { ok: true, data: normalizeFocusActiveState(result.data) } : result;
  });
  ipcMain.handle("manager.upsertFocusBlocklist", async (_event, value) => {
    const input = value && typeof value === "object" ? (value as Record<string, unknown>) : {};
    const domains = input.domains;
    const kind = input.kind === "raw" ? "raw" : input.kind === "domains" ? "domains" : null;
    if (
      (input.id !== undefined && !validFocusId(input.id)) ||
      !bounded(input.name, 256) ||
      !kind ||
      !Array.isArray(domains) ||
      domains.length > 512 ||
      domains.some(
        (entry) => typeof entry !== "string" || entry.trim().length === 0 || entry.length > 512,
      ) ||
      (input.icon !== undefined && (typeof input.icon !== "string" || input.icon.length > 16)) ||
      (input.preset !== undefined && typeof input.preset !== "boolean")
    )
      return {
        ok: false,
        code: "validation",
        message: "Недопустимые данные блок-листа.",
      };
    if (typeof input.id === "string") {
      const active = await rpc(op.getFocusActiveState);
      if (!active.ok || !isValidFocusActiveState(active.data))
        return {
          ok: false,
          code: "engine",
          message: "Не удалось проверить активное состояние.",
        };
      if (isActiveFocusBlocklist(active.data, input.id))
        return {
          ok: false,
          code: "validation",
          message: "Нельзя изменить активный блок-лист.",
        };
    }
    const result = await rpc(op.upsertFocusBlocklist, {
      ...(typeof input.id === "string" ? { id: input.id } : {}),
      name: input.name,
      domains,
      kind,
      ...(typeof input.icon === "string" ? { icon: input.icon } : {}),
      ...(typeof input.preset === "boolean" ? { preset: input.preset } : {}),
    });
    return result.ok
      ? {
          ok: true,
          data: normalizeFocusBlocklists({ blocklists: [result.data] })[0],
        }
      : result;
  });
  ipcMain.handle("manager.deleteFocusBlocklist", async (_event, value) => {
    const id = value && typeof value === "object" ? (value as { id?: unknown }).id : undefined;
    if (!validFocusId(id))
      return {
        ok: false,
        code: "validation",
        message: "Недопустимый идентификатор блок-листа.",
      };
    const active = await rpc(op.getFocusActiveState);
    if (!active.ok || !isValidFocusActiveState(active.data))
      return {
        ok: false,
        code: "engine",
        message: "Не удалось проверить активное состояние.",
      };
    if (isActiveFocusBlocklist(active.data, id))
      return {
        ok: false,
        code: "validation",
        message: "Нельзя удалить активный блок-лист.",
      };
    const result = await rpc(op.deleteFocusBlocklist, { id });
    return result.ok ? { ok: true, data: { deleted: true } } : result;
  });
  ipcMain.handle("manager.getFocusServiceStatus", async () => {
    if (process.env.KOSMOS_TEST_FOCUS_SERVICE === "absent")
      return {
        ok: true,
        data: { installed: false, running: false, healthy: false },
      };
    if (process.env.KOSMOS_TEST_FOCUS_SERVICE === "installed")
      return {
        ok: true,
        data: { installed: true, running: true, healthy: true },
      };
    try {
      const status = await getServiceStatus();
      const healthy = status.running && (await pingService());
      return {
        ok: true,
        data: normalizeFocusServiceStatus({ ...status, healthy }),
      };
    } catch {
      return {
        ok: false,
        code: "engine",
        message: "Не удалось проверить системную службу.",
      };
    }
  });
  ipcMain.handle("manager.pingFocusService", async () => {
    if (process.env.KOSMOS_TEST_FOCUS_SERVICE === "installed")
      return { ok: true, data: { healthy: true } };
    try {
      return { ok: true, data: { healthy: await pingService() } };
    } catch {
      return { ok: true, data: { healthy: false } };
    }
  });
  for (const [channel, subcommand] of [
    ["manager.installFocusService", "install"],
    ["manager.uninstallFocusService", "uninstall"],
    ["manager.startFocusService", "start"],
    ["manager.stopFocusService", "stop"],
  ] as const) {
    ipcMain.handle(channel, async () => {
      if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1")
        return process.env.KOSMOS_TEST_FOCUS_SERVICE_ACTION === "ok"
          ? { ok: true, data: { ok: true } }
          : { ok: true, data: { ok: false, error: "headless" } };
      const result = await runServiceCliElevated(subcommand);
      return {
        ok: true,
        data: result.ok ? { ok: true } : { ok: false, error: "Служба не выполнила действие." },
      };
    });
  }
  register(
    "manager.disconnectPeer",
    op.disconnectPeer,
    (value) =>
      validation(
        typeof value === "object" && bounded((value as { peer_id?: unknown }).peer_id, 256),
      ),
    (value) => toDeviceParams((value as { peer_id: string }).peer_id),
  );
  ipcMain.handle("manager.getPackages", async (_event, value) => {
    if (
      value !== undefined &&
      (!isObject(value) ||
        Object.keys(value).length !== 1 ||
        !["app", "source", "bridge"].includes(String(value.kind)))
    )
      return {
        ok: false,
        code: "validation",
        message: "Недопустимый тип пакета.",
      };
    const result = await rpc(op.getPackages, value as Record<string, unknown> | undefined);
    return result.ok ? { ok: true, data: normalizePackageSnapshot(result.data) } : result;
  });
  register("manager.getStoreCatalog", op.getStoreCatalog);
  register("manager.refreshStoreCatalog", op.refreshStoreCatalog);
  ipcMain.handle("manager.openStoreExternal", async (_event, value) => {
    if (!isObject(value) || !bounded(value.listing_id, 128))
      return {
        ok: false,
        code: "validation",
        message: "Некорректное приложение магазина.",
      };
    const result = await rpc(op.openStoreExternal, {
      listing_id: value.listing_id,
    });
    if (!result.ok) return result;
    const raw =
      typeof result.data === "string"
        ? result.data
        : (result.data as { official_url?: unknown })?.official_url;
    if (typeof raw !== "string")
      return { ok: false, code: "engine", message: "Движок не вернул ссылку." };
    let url: URL;
    try {
      url = new URL(raw);
    } catch {
      return {
        ok: false,
        code: "validation",
        message: "Ссылка приложения некорректна.",
      };
    }
    if (url.protocol !== "https:")
      return {
        ok: false,
        code: "validation",
        message: "Разрешены только HTTPS-ссылки.",
      };
    await shell.openExternal(url.toString());
    return { ok: true, data: { opened: true } };
  });
  register(
    "manager.getBridgeConfig",
    op.getBridgeConfig,
    (value) =>
      validation(
        isObject(value) &&
          Object.keys(value).length === 2 &&
          validPackageId(value.package_id) &&
          validPackageVersion(value.version),
      ),
    (value) => ({
      id: (value as { package_id: string }).package_id,
      version: (value as { version: string }).version,
    }),
  );
  register(
    "manager.setBridgeConfig",
    op.setBridgeConfig,
    (value) => {
      const input = value as {
        package_id?: unknown;
        version?: unknown;
        config?: {
          vault_root?: unknown;
          selected_types?: unknown;
          editable_fields?: unknown;
          readonly_fields?: unknown;
        };
      };
      const fields = [
        input.config?.selected_types,
        input.config?.editable_fields,
        input.config?.readonly_fields,
      ];
      return validation(
        isObject(value) &&
          Object.keys(value).length === 3 &&
          validPackageId(input.package_id) &&
          validPackageVersion(input.version) &&
          isObject(input.config) &&
          Object.keys(input.config).length === 4 &&
          bounded(input.config?.vault_root, 4096) &&
          fields.every(
            (field) =>
              Array.isArray(field) &&
              field.length <= 64 &&
              field.every((name) => bounded(name, 128)),
          ),
      );
    },
    (value) => {
      const input = value as {
        package_id: string;
        version: string;
        config: {
          vault_root: string;
          selected_types: string[];
          editable_fields: string[];
          readonly_fields: string[];
        };
      };
      return {
        id: input.package_id,
        version: input.version,
        config: input.config,
      };
    },
  );
  ipcMain.handle("manager.getPackageTrustStatus", async () => {
    const result = await rpc(op.getPackageTrustStatus);
    return result.ok ? { ok: true, data: normalizePackageTrustStatus(result.data) } : result;
  });
  ipcMain.handle("manager.refreshPackageCatalog", async (_event, value) => {
    if (value !== undefined)
      return {
        ok: false,
        code: "validation",
        message: "Недопустимые параметры.",
      };
    const result = await rpc(op.refreshPackageCatalog);
    return result.ok ? { ok: true, data: { refreshed: true } } : result;
  });
  const packageInput = (value: unknown, enabled = false) => {
    if (!isObject(value)) return null;
    const expected = enabled ? 3 : 2;
    if (
      Object.keys(value).length !== expected ||
      !validPackageId(value.package_id) ||
      !validPackageVersion(value.version) ||
      (enabled && typeof value.enabled !== "boolean")
    )
      return null;
    return {
      id: value.package_id,
      version: value.version,
      ...(enabled ? { enabled: value.enabled } : {}),
    };
  };
  ipcMain.handle("manager.installPackage", async (_event, value) => {
    const input = packageInput(value);
    if (!input)
      return {
        ok: false,
        code: "validation",
        message: "Недопустимые данные пакета.",
      };
    const result = await rpc(op.installPackage, input);
    if (!result.ok) return result;
    const packages = await rpc(op.getPackages);
    const installed =
      packages.ok && isObject(packages.data) && Array.isArray(packages.data.packages)
        ? packages.data.packages.find((item) => isObject(item) && item.id === input.id)
        : null;
    if (isObject(installed) && installed.kind === "app") {
      const enabled = await rpc(op.setPackageEnabled, {
        id: input.id,
        version: input.version,
        enabled: true,
      });
      if (!enabled.ok) return enabled;
      if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1")
        return {
          ok: true,
          data: { installed: true, enabled: true, opened: false },
        };
      if (!openHostedPackage(input.id))
        return {
          ok: false,
          code: "engine",
          message: "Приложение установлено, но Host недоступен.",
        };
      return {
        ok: true,
        data: { installed: true, enabled: true, opened: true },
      };
    }
    return { ok: true, data: { installed: true } };
  });
  ipcMain.handle("manager.openPackage", async (_event, value) => {
    if (!isObject(value) || Object.keys(value).length !== 1 || !validPackageId(value.package_id))
      return {
        ok: false,
        code: "validation",
        message: "Некорректное приложение.",
      };
    return openHostedPackage(value.package_id)
      ? { ok: true, data: { opened: true } }
      : { ok: false, code: "engine", message: "Package Host недоступен." };
  });
  ipcMain.handle("manager.setPackageEnabled", async (_event, value) => {
    const input = packageInput(value, true);
    if (!input)
      return {
        ok: false,
        code: "validation",
        message: "Недопустимые данные пакета.",
      };
    const result = await rpc(op.setPackageEnabled, input);
    return result.ok ? { ok: true, data: { enabled: input.enabled === true } } : result;
  });
  ipcMain.handle("manager.uninstallPackage", async (_event, value) => {
    const input = packageInput(value);
    if (!input)
      return {
        ok: false,
        code: "validation",
        message: "Недопустимые данные пакета.",
      };
    const result = await rpc(op.uninstallPackage, input);
    return result.ok ? { ok: true, data: { uninstalled: true } } : result;
  });
  register("manager.getDiagnosticsSnapshot", op.getDiagnosticsSnapshot);
  register("manager.getDiagnosticLogTail", op.getDiagnosticLogTail);
  const dataRoot = () => resolveManagerDataDir(app.getPath("appData"));
  ipcMain.handle("manager.listCrashReports", async () => {
    try {
      return { ok: true, data: await listCrashReports(dataRoot()) };
    } catch {
      return {
        ok: false,
        code: "engine",
        message: "Не удалось прочитать отчёты об ошибках.",
      };
    }
  });
  ipcMain.handle("manager.clearCrashReports", async () => {
    try {
      return {
        ok: true,
        data: { removed: await clearCrashReports(dataRoot()) },
      };
    } catch {
      return {
        ok: false,
        code: "engine",
        message: "Не удалось очистить отчёты об ошибках.",
      };
    }
  });
  async function openFolder(kind: "logs" | "crashes") {
    if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1")
      return { ok: true, data: { opened: false } } as const;
    try {
      const dir = await ensureDiagnosticsDir(dataRoot(), kind);
      const error = await shell.openPath(dir);
      return { ok: true, data: { opened: !error } } as const;
    } catch {
      return {
        ok: false,
        code: "engine",
        message: "Не удалось открыть папку диагностики.",
      } as const;
    }
  }
  ipcMain.handle("manager.openCrashReportsFolder", () => openFolder("crashes"));
  ipcMain.handle("manager.openLogsFolder", () => openFolder("logs"));
  const appExecutable = process.env.KOSMOS_APP_EXECUTABLE?.trim();
  const loginItemOptions = { path: appExecutable ?? "", args: AUTOSTART_ARGS };
  const autostartAvailable = () =>
    app.isPackaged &&
    process.platform === "win32" &&
    Boolean(appExecutable) &&
    resolveInstance().autorunEnabled;
  ipcMain.handle("manager.getAutostart", () => {
    if (!autostartAvailable()) return { ok: true, data: { enabled: false, available: false } };
    const settings = app.getLoginItemSettings(loginItemOptions);
    return { ok: true, data: { enabled: settings.openAtLogin, available: true } };
  });
  ipcMain.handle("manager.setAutostart", (_event, value) => {
    if (!isObject(value) || typeof value.enabled !== "boolean")
      return { ok: false, code: "validation", message: "Недопустимое значение автозапуска." };
    if (!autostartAvailable()) return { ok: true, data: { enabled: false, available: false } };
    app.setLoginItemSettings({ ...loginItemOptions, openAtLogin: value.enabled });
    const settings = app.getLoginItemSettings(loginItemOptions);
    return { ok: true, data: { enabled: settings.openAtLogin, available: true } };
  });
  ipcMain.handle("manager.getEngineSettings", async () => {
    const result = await rpc(op.getEngineSettings);
    return result.ok ? { ok: true, data: normalizeEngineSettings(result.data) } : result;
  });
  register(
    "manager.setWarmTimeout",
    op.setWarmTimeout,
    (value) =>
      validation(
        typeof value === "object" && typeof (value as { enabled?: unknown }).enabled === "boolean",
      ),
    (value) => ({
      warm_timeout_seconds: (value as { enabled: boolean }).enabled ? 300 : 0,
    }),
  );
  ipcMain.handle("manager.setUsageTracker", async (_event, value) => {
    if (!validateUsageTrackerSettingsPatch(value)) {
      return {
        ok: false,
        code: "validation",
        message: "Недопустимое значение учёта активности.",
      };
    }
    const result = await rpc(op.setUsageTracker, {
      usage_tracker_enabled: value.enabled,
    });
    return result.ok ? { ok: true, data: normalizeEngineSettings(result.data) } : result;
  });

  const readFileIndexSettings = async (): Promise<ManagerResult<unknown>> => {
    const result = await rpc(op.getFileIndexSettings);
    return result.ok ? { ok: true, data: normalizeFileIndexSettings(result.data) } : result;
  };
  ipcMain.handle("manager.getFileIndexSettings", readFileIndexSettings);
  ipcMain.handle("manager.setFileIndexSettings", async (_event, value) => {
    const patch = validateFileIndexSettingsPatch(value);
    if (!patch)
      return {
        ok: false,
        code: "validation",
        message: "Недопустимые настройки индекса файлов.",
      };
    const updated = await rpc(op.setFileIndexSettings, patch);
    if (!updated.ok) return updated;
    return readFileIndexSettings();
  });
  ipcMain.handle("manager.getFileIndexDiagnostics", async () => {
    const result = await rpc(op.getFileIndexDiagnostics);
    return result.ok ? { ok: true, data: normalizeFileIndexDiagnostics(result.data) } : result;
  });
  const fileIndexMutation = async (
    operation: string,
    value: unknown,
    field: "path" | "pattern",
  ): Promise<ManagerResult<unknown>> => {
    if (!isObject(value) || !validateFileIndexPath(value[field], field === "path" ? 4096 : 512))
      return {
        ok: false,
        code: "validation",
        message: field === "path" ? "Выберите папку индексации." : "Введите шаблон исключения.",
      };
    const updated = await rpc(operation, {
      [field]: String(value[field]).trim(),
    });
    if (!updated.ok) return updated;
    return readFileIndexSettings();
  };
  ipcMain.handle("manager.addFileIndexRoot", (_event, value) =>
    fileIndexMutation(op.addFileIndexRoot, value, "path"),
  );
  ipcMain.handle("manager.removeFileIndexRoot", (_event, value) =>
    fileIndexMutation(op.removeFileIndexRoot, value, "path"),
  );
  ipcMain.handle("manager.addFileIndexIgnore", (_event, value) =>
    fileIndexMutation(op.addFileIndexIgnore, value, "pattern"),
  );
  ipcMain.handle("manager.removeFileIndexIgnore", (_event, value) =>
    fileIndexMutation(op.removeFileIndexIgnore, value, "pattern"),
  );
  for (const [channel, operation] of [
    ["manager.rescanFileIndex", op.rescanFileIndex],
    ["manager.clearFileIndexCache", op.clearFileIndexCache],
  ] as const) {
    ipcMain.handle(channel, async () => {
      const updated = await rpc(operation);
      return updated.ok ? readFileIndexSettings() : updated;
    });
  }
  ipcMain.handle("manager.pickFileIndexRoot", async (event) => {
    const senderWindow = BrowserWindow.fromWebContents(event.sender);
    if (!managerWindow || senderWindow !== managerWindow)
      return {
        ok: false,
        code: "validation",
        message: "Недопустимый источник выбора папки.",
      };
    if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1")
      return { ok: true, data: null };
    try {
      const options: OpenDialogOptions = {
        title: "Выберите папку для индексации",
        properties: ["openDirectory", "createDirectory"],
      };
      const picked = await dialog.showOpenDialog(senderWindow, options);
      return {
        ok: true,
        data: picked.canceled ? null : (picked.filePaths[0] ?? null),
      };
    } catch {
      return {
        ok: false,
        code: "engine",
        message: "Не удалось открыть выбор папки.",
      };
    }
  });

  ipcMain.handle("manager.saveSupportBundle", async () => {
    const created = await rpc(op.createSupportBundle);
    if (!created.ok) return created;
    const handle = (created.data as { handle?: unknown })?.handle;
    if (!bounded(handle, 256)) {
      return {
        ok: false,
        code: "engine",
        message: "Engine вернул некорректный пакет поддержки.",
      };
    }
    if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") {
      await rpc(op.cancelSupportBundle, { handle });
      return { ok: true, data: { saved: false, cancelled: true } };
    }
    const picked = await dialog.showSaveDialog({
      title: "Сохранить пакет поддержки",
      defaultPath: "kosmos-support.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (picked.canceled || !picked.filePath) {
      await rpc(op.cancelSupportBundle, { handle });
      return { ok: true, data: { saved: false, cancelled: true } };
    }
    const saved = await rpc(op.saveSupportBundle, {
      handle,
      destination: picked.filePath,
    });
    return saved.ok ? { ok: true, data: { saved: true } } : saved;
  });
}

let managerWindow: BrowserWindow | null = null;
let managerWindowReady = false;

function presentManagerWindow(win: BrowserWindow) {
  if (
    !managerWindowReady ||
    win.isDestroyed() ||
    process.env.KOSMOS_HEADLESS === "1" ||
    process.env.KOSMOS_TEST_MODE === "1"
  )
    return;
  if (win.isMinimized()) win.restore();
  win.show();
  win.focus();
}

async function createWindow() {
  if (managerWindow && !managerWindow.isDestroyed()) {
    presentManagerWindow(managerWindow);
    return managerWindow;
  }
  const headless = process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
  const win = new BrowserWindow({
    width: 1180,
    height: 760,
    show: false,
    skipTaskbar: headless,
    titleBarStyle: "hidden",
    titleBarOverlay: {
      color: "#00000000",
      symbolColor: "#FFFFFF",
      height: 41,
    },
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      offscreen: process.env.KOSMOS_CAPTURE_OFFSCREEN === "1",
    },
  });
  managerWindow = win;
  managerWindowReady = false;
  win.once("ready-to-show", () => {
    if (managerWindow !== win || win.isDestroyed()) return;
    managerWindowReady = true;
    presentManagerWindow(win);
  });
  win.on("closed", () => {
    if (managerWindow === win) {
      managerWindow = null;
      managerWindowReady = false;
    }
    // Manager owns no Engine process; closing/crashing this window is isolated.
  });
  win.webContents.on("render-process-gone", () => {
    if (!win.isDestroyed()) win.destroy();
    if (managerWindow === win) {
      managerWindow = null;
      managerWindowReady = false;
    }
  });
  win.webContents.on("did-fail-load", (_event, _code, _description, _url, isMainFrame) => {
    if (!isMainFrame) return;
    if (!win.isDestroyed()) win.destroy();
    if (managerWindow === win) {
      managerWindow = null;
      managerWindowReady = false;
    }
  });
  const devUrl = process.env.VITE_DEV_SERVER_URL;
  try {
    await (devUrl ? win.loadURL(devUrl) : win.loadFile(path.join(__dirname, "../dist/index.html")));
  } catch (error) {
    if (!win.isDestroyed()) win.destroy();
    if (managerWindow === win) {
      managerWindow = null;
      managerWindowReady = false;
    }
    throw error;
  }
  return win;
}

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on("second-instance", () => {
    if (
      !app.isReady() ||
      process.env.KOSMOS_HEADLESS === "1" ||
      process.env.KOSMOS_TEST_MODE === "1"
    )
      return;
    void createWindow().then((win) => {
      presentManagerWindow(win);
    });
  });
  app.whenReady().then(async () => {
    Menu.setApplicationMenu(null);
    session.defaultSession.webRequest.onHeadersReceived((details, callback) =>
      callback({
        responseHeaders: {
          ...details.responseHeaders,
          "Content-Security-Policy": [
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'",
          ],
        },
      }),
    );
    registerAll();
    if (process.env.KOSMOS_HEADLESS !== "1" && process.env.KOSMOS_TEST_MODE !== "1")
      await waitForEngineReady();
    await createWindow();
  });
  app.on("activate", () => void createWindow());
  app.on("window-all-closed", () => {
    // Closing Manager exits only this Electron process; Engine is independent.
    if (process.platform !== "darwin") app.quit();
  });
}
