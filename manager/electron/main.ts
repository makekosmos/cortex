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
import { statSync } from "node:fs";
import path from "node:path";
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
  isString,
  isBoolean,
  type Input,
  type InputRecord,
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
import { clearIntegrationBrowserData, registerIntegrationLoginHandlers } from "./integration-login";
import { readBrowserDataPersistence, writeBrowserDataPersistence } from "./browser-settings";
import { resolveInstance } from "../../desktop/electron/instance";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
import {
  AUTOSTART_ARGS,
  bounded,
  isObject,
  normalizeIntegrationSnapshot,
  openHostedPackage,
  readDesktopUpdateState,
  requestDesktopUpdate,
  startPackagedRuntime,
  validIntegrationInput,
  validation,
} from "./main-helpers";
import { developmentPackage, developmentPackages } from "./dev-packages";
let dictationSubscriber: WebContents | null = null;
let stopDictationEvents: (() => void) | null = null;

function desktopVersion(): string {
  const launchedVersion = process.env.KOSMOS_DESKTOP_VERSION?.trim();
  if (launchedVersion) return launchedVersion;
  return path.basename(process.execPath).toLowerCase() === "electron.exe" ? "—" : app.getVersion();
}

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
  validate?: (value: Input) => string | null,
  transform?: (value: Input) => InputRecord,
) {
  ipcMain.handle(name, async (_event, value) => {
    const error = validate?.(value);
    if (error) return { ok: false, code: "validation", message: error };
    return rpc(operation, transform ? transform(value) : value);
  });
}

function registerAll() {
  const browserSettingsFile = path.join(app.getPath("userData"), "browser.json");
  ipcMain.handle("manager.getAppVersion", () => ({
    ok: true,
    data: desktopVersion(),
  }));
  ipcMain.handle("manager.getDesktopUpdateState", () => {
    const state = readDesktopUpdateState();
    return { ok: true, data: state ?? { kind: "idle" } };
  });
  ipcMain.handle("manager.checkDesktopUpdates", () => {
    if (!requestDesktopUpdate("--kosmos-update-check")) return { ok: true, data: { kind: "idle" } };
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
      : {
          ok: false,
          code: "engine",
          message: "Kosmos Desktop недоступен для перезапуска.",
        };
  });
  ipcMain.handle("manager.getHealth", async () => status("/v1/health"));
  ipcMain.handle("manager.getInfo", async () => status("/v1/info"));
  register("manager.getDataSummary", op.getDataSummary);
  register("manager.listObjectTypes", op.listObjectTypes);
  register("manager.listObjects", op.listObjects, (value) =>
    validation(
      // SAFETY: assertions below read optional fields only after the object shape is checked.
      value === undefined ||
        (isObject(value) &&
          value !== null &&
          (!("type_id" in value) ||
            (value as InputRecord).type_id === undefined ||
            bounded((value as InputRecord).type_id, 128)) &&
          (!("cursor" in value) ||
            (value as InputRecord).cursor === undefined ||
            (isString((value as InputRecord).cursor) &&
              String((value as InputRecord).cursor).length <= 20)) &&
          (!("limit" in value) ||
            (Number.isInteger((value as InputRecord).limit) &&
              Number((value as InputRecord).limit) >= 1 &&
              Number((value as InputRecord).limit) <= 200))),
    ),
  );
  register("manager.searchObjects", op.searchObjects, (value) =>
    // SAFETY: isObject establishes a record payload before reading query.
    validation(isObject(value) && bounded((value as InputRecord).query, 256)),
  );
  ipcMain.handle("manager.getSyncSnapshot", async (): Promise<ManagerResult<SyncSnapshot>> => {
    const result = await rpc(op.getSyncSnapshot);
    return result.ok ? { ok: true, data: normalizeSyncSnapshot(result.data) } : result;
  });
  ipcMain.handle("manager.getPairingCode", async () => {
    const result = await rpc(op.getPairingCode);
    if (!result.ok) return result;
    const rawCode =
      // SAFETY: RPC pairing response is either a string code or a record with code.
      isString(result.data) ? result.data : (result.data as InputRecord)?.code;
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
      // SAFETY: isObject narrows the payload before reading the pairing code.
      validation(isObject(value) && validPairingCode((value as InputRecord).code)),
    // SAFETY: the validator requires a valid pairing code before transformation.
    (value) => toPairingParams(String((value as InputRecord).code)),
  );
  ipcMain.handle("manager.getIntegrations", async () => {
    const result = await rpc(op.getIntegrations);
    return result.ok ? { ok: true, data: normalizeIntegrationSnapshot(result.data) } : result;
  });
  registerIntegrationLoginHandlers(
    () => managerWindow,
    () => readBrowserDataPersistence(browserSettingsFile),
  );
  ipcMain.handle("manager.getBrowserSettings", () => ({
    ok: true,
    data: { persistData: readBrowserDataPersistence(browserSettingsFile) },
  }));
  ipcMain.handle("manager.setBrowserSettings", async (_event, value) => {
    if (!isObject(value) || Object.keys(value).length !== 1 || !isBoolean(value.persistData))
      return {
        ok: false,
        code: "validation",
        message: "Недопустимая настройка браузера.",
      };
    writeBrowserDataPersistence(browserSettingsFile, value.persistData);
    if (!value.persistData) await clearIntegrationBrowserData();
    return { ok: true, data: { persistData: value.persistData } };
  });
  ipcMain.handle("manager.setIntegrationCredential", async (_event, value) => {
    const credential = isObject(value) ? value.credential : undefined;
    const setting = isObject(value) ? value.setting : undefined;
    if (
      !validIntegrationInput(value) ||
      !isString(credential) ||
      !credential ||
      credential.length > 4096 ||
      (setting !== undefined && !bounded(setting, 128))
    )
      return {
        ok: false,
        code: "validation",
        message: "Введите корректные данные подключения.",
      };
    const input: InputRecord = {
      provider: value.provider,
      credential: credential.trim(),
    };
    if (setting !== undefined) input.setting = setting;
    const result = await rpc(op.setIntegrationCredential, input);
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
      // SAFETY: isObject narrows the successful RPC payload to a record.
      const raw = result.ok && isObject(result.data) ? (result.data as InputRecord) : {};
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
      const input = isObject(value) ? value : {};
      const key = input.key;
      if (Object.keys(input).length !== 1 || !bounded(key, 512))
        return { ok: false, code: "validation", message: "Введите ключ." };
      const result = await rpc(operation, { key });
      // SAFETY: isObject narrows the successful RPC payload to a record.
      const raw = result.ok && isObject(result.data) ? (result.data as InputRecord) : {};
      return result.ok
        ? {
            ok: true,
            data:
              operation === op.verifyDictationApiKey
                ? {
                    valid: raw.ok === true,
                    message: isString(raw.reason)
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
      const input = isObject(value) ? value : {};
      const v = input[field];
      if (!validDictationId(v))
        return {
          ok: false,
          code: "validation",
          message: "Недопустимое значение.",
        };
      const params: InputRecord = { [field]: v };
      if (field === "modelId") {
        if (
          Object.keys(input).some((key) => key !== "modelId" && key !== "select") ||
          ("select" in input && !isBoolean(input.select))
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
    const input = isObject(value) ? value : {};
    const domains = input.domains;
    const kind = input.kind === "raw" ? "raw" : input.kind === "domains" ? "domains" : null;
    if (
      (input.id !== undefined && !validFocusId(input.id)) ||
      !bounded(input.name, 256) ||
      !kind ||
      !Array.isArray(domains) ||
      domains.length > 512 ||
      domains.some(
        (entry: Input) => !isString(entry) || entry.trim().length === 0 || entry.length > 512,
      ) ||
      (input.icon !== undefined && (!isString(input.icon) || input.icon.length > 16)) ||
      (input.preset !== undefined && !isBoolean(input.preset))
    )
      return {
        ok: false,
        code: "validation",
        message: "Недопустимые данные блок-листа.",
      };
    if (isString(input.id)) {
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
    const params: InputRecord = { name: input.name, domains, kind };
    if (isString(input.id)) params.id = input.id;
    if (isString(input.icon)) params.icon = input.icon;
    if (isBoolean(input.preset)) params.preset = input.preset;
    const result = await rpc(op.upsertFocusBlocklist, params);
    return result.ok
      ? {
          ok: true,
          data: normalizeFocusBlocklists({
            blocklists: [result.data ?? null],
          })[0],
        }
      : result;
  });
  ipcMain.handle("manager.deleteFocusBlocklist", async (_event, value) => {
    const id = isObject(value) ? value.id : undefined;
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
    (value) => validation(isObject(value) && bounded(value.peer_id, 256)),
    // SAFETY: the validator requires a bounded peer_id string.
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
    // SAFETY: validation above restricts the optional package filter object.
    const result = await rpc(op.getPackages, value as InputRecord | undefined);
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
    // SAFETY: successful RPC payload is validated as string or record before URL parsing.
    const raw = isString(result.data) ? result.data : (result.data as InputRecord)?.official_url;
    if (!isString(raw)) return { ok: false, code: "engine", message: "Движок не вернул ссылку." };
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
    // SAFETY: validator requires package_id and version fields.
    (value) => ({
      package_id: (value as { package_id: string }).package_id,
      version: (value as { version: string }).version,
    }),
  );
  register(
    "manager.setBridgeConfig",
    op.setBridgeConfig,
    (value) => {
      // SAFETY: validation branch below checks the complete bridge config shape.
      const input = value as {
        package_id?: string;
        version?: string;
        config?: {
          vault_root?: string;
          selected_types?: string[];
          editable_fields?: string[];
          readonly_fields?: string[];
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
          Object.keys(input.config ?? {}).length === 4 &&
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
      // SAFETY: the preceding validator guarantees these bridge config fields.
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
        package_id: input.package_id,
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
  const packageInput = (value: Input, enabled = false) => {
    if (!isObject(value)) return null;
    const expected = enabled ? 3 : 2;
    if (
      Object.keys(value).length !== expected ||
      !validPackageId(value.package_id) ||
      !validPackageVersion(value.version) ||
      (enabled && !isBoolean(value.enabled))
    )
      return null;
    const result: InputRecord = {
      package_id: value.package_id,
      version: value.version,
    };
    if (enabled) result.enabled = value.enabled;
    return result;
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
        ? packages.data.packages.find(
            (item: Input) => isObject(item) && item.id === input.package_id,
          )
        : null;
    if (isObject(installed) && installed.kind === "app") {
      const enabled = await rpc(op.setPackageEnabled, {
        package_id: input.package_id,
        version: input.version,
        enabled: true,
      });
      if (!enabled.ok) return enabled;
      if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1")
        return {
          ok: true,
          data: { installed: true, enabled: true, opened: false },
        };
      if (!openHostedPackage(String(input.package_id)))
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
  ipcMain.handle("manager.getDevelopmentPackages", () => ({
    ok: true,
    data: developmentPackages().map(({ archivePath: _archivePath, iconPath, url, ...entry }) => {
      const iconUrl = new URL(path.basename(iconPath), url);
      iconUrl.searchParams.set("v", String(statSync(iconPath).mtimeMs));
      return { ...entry, icon_url: iconUrl.toString() };
    }),
  }));
  ipcMain.handle("manager.openDevelopmentPackage", async (_event, value) => {
    if (!isObject(value) || Object.keys(value).length !== 1 || !validPackageId(value.package_id))
      return { ok: false, code: "validation", message: "Invalid development package." };
    const development = developmentPackage(value.package_id);
    if (!development)
      return { ok: false, code: "validation", message: "Development package is not running." };
    const installed = await rpc("packages.install_development", {
      package_id: development.id,
      version: development.version,
      archive_path: development.archivePath,
    });
    if (!installed.ok) return installed;
    return openHostedPackage(development.id, development)
      ? { ok: true, data: { opened: true } }
      : { ok: false, code: "engine", message: "Package Host unavailable." };
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
    return {
      ok: true,
      data: { enabled: settings.openAtLogin, available: true },
    };
  });
  ipcMain.handle("manager.setAutostart", (_event, value) => {
    if (!isObject(value) || !isBoolean(value.enabled))
      return {
        ok: false,
        code: "validation",
        message: "Недопустимое значение автозапуска.",
      };
    if (!autostartAvailable()) return { ok: true, data: { enabled: false, available: false } };
    app.setLoginItemSettings({
      ...loginItemOptions,
      openAtLogin: value.enabled,
    });
    const settings = app.getLoginItemSettings(loginItemOptions);
    return {
      ok: true,
      data: { enabled: settings.openAtLogin, available: true },
    };
  });
  ipcMain.handle("manager.getEngineSettings", async () => {
    const result = await rpc(op.getEngineSettings);
    return result.ok ? { ok: true, data: normalizeEngineSettings(result.data) } : result;
  });
  register(
    "manager.setWarmTimeout",
    op.setWarmTimeout,
    (value) => validation(isObject(value) && isBoolean(value.enabled)),
    // SAFETY: the validator requires enabled to be boolean.
    (value) => ({
      warm_timeout_seconds: (value as InputRecord).enabled ? 300 : 0,
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
    value: Input,
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
    // SAFETY: successful support-bundle response carries an optional handle field.
    const handle = (created.data as InputRecord)?.handle;
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
const styleSource = process.env.VITE_DEV_SERVER_URL
  ? "style-src 'self' 'unsafe-inline'"
  : "style-src 'self'";
const imageSource =
  process.env.KOSMOS_DEV_PACKAGES === "1"
    ? "img-src 'self' data: https: http://127.0.0.1:* http://localhost:*"
    : "img-src 'self' data: https:";

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
            `default-src 'self'; script-src 'self'; ${styleSource}; ${imageSource}; font-src 'self'`,
          ],
        },
      }),
    );
    registerAll();
    if (process.env.KOSMOS_HEADLESS !== "1" && process.env.KOSMOS_TEST_MODE !== "1") {
      startPackagedRuntime();
      void waitForEngineReady();
    }
    await createWindow();
  });
  app.on("activate", () => void createWindow());
  app.on("window-all-closed", () => {
    // Closing Manager exits only this Electron process; Engine is independent.
    if (process.platform !== "darwin") app.quit();
  });
}
