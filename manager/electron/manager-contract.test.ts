import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import {
  normalizeConnectivity,
  normalizeFocusActiveState,
  normalizeFocusBlocklists,
  isActiveFocusBlocklist,
  isValidFocusActiveState,
  normalizeDictationConfig,
  normalizeDictationEvent,
  normalizeDictationLocalModels,
  normalizeDictationPending,
  normalizeDictationStats,
  normalizeEngineSettings,
  normalizeFileIndexDiagnostics,
  normalizeFileIndexSettings,
  normalizePackageSnapshot,
  normalizePackageTrustStatus,
  validPackageId,
  validPackageVersion,
  validateFileIndexPath,
  validateFileIndexSettingsPatch,
  validateUsageTrackerSettingsPatch,
  normalizePairingCode,
  normalizeSyncSnapshot,
  toDeviceParams,
  toPairingParams,
  validateDictationConfigPatch,
  validPairingCode,
} from "./manager-contract";

const source = (name: string) => readFileSync(join(import.meta.dir, name), "utf8");

describe("standalone Manager boundary", () => {
  test("keeps Manager navigation stable and autostart targets Desktop", () => {
    const root = source("../src/ManagerRoot.vue");
    const settings = source("../src/views/SettingsView.vue");
    const main = source("main.ts");
    const navigation = source("../../desktop/electron/manager-navigation.ts");
    expect(root).toContain("<KeepAlive>");
    expect(root).not.toContain("Центр управления");
    expect(root).not.toContain(">Обновить</Button>");
    expect(root).toContain("scrollPositions");
    expect(settings).toContain("Запускать Kosmos при входе в систему");
    expect(main).toContain("app.setLoginItemSettings");
    expect(main).toContain("KOSMOS_APP_EXECUTABLE");
    expect(main).toContain("args: AUTOSTART_ARGS");
    expect(main).toContain("resolveInstance().autorunEnabled");
    expect(main.indexOf("resolveInstance().autorunEnabled")).toBeLessThan(
      main.indexOf("app.setLoginItemSettings"),
    );
    expect(navigation).toContain("KOSMOS_APP_EXECUTABLE: process.execPath");
  });

  test("exposes validated Manager connections and package app handoff", () => {
    const api = source("../src/manager-api.ts");
    const preload = source("preload.ts");
    const main = source("main.ts");
    const helpers = source("main-helpers.ts");
    const operations = source("../src/manager-api.ts");
    const root = source("../src/ManagerRoot.vue");
    expect(api).toContain("getIntegrations");
    expect(preload).toContain("manager.getIntegrations");
    expect(main).toContain("normalizeIntegrationSnapshot");
    expect(operations).toContain("integrations.update_settings");
    expect(helpers).toContain("--open-app=${id}");
    expect(api).toContain("openPackage");
    expect(preload).toContain('openPackage: (v) => invoke("manager.openPackage", v)');
    expect(main).toContain('ipcMain.handle("manager.openPackage"');
    expect(root).toContain("Интеграции");
  });

  test("uses the Desktop Host sibling, refreshes integrations after sync, and keeps one Manager", () => {
    const main = source("main.ts");
    const helpers = source("main-helpers.ts");
    const hostResolution = source("host-resolution.ts");
    const sync = main.slice(
      main.indexOf('ipcMain.handle("manager.syncIntegrationNow"'),
      main.indexOf('ipcMain.handle("manager.getDictationConfig"'),
    );
    expect(hostResolution).toContain('"..", "..", "host"');
    expect(helpers).toContain("resolvePackagedHostExecutable(process.resourcesPath)");
    expect(main).toContain('process.env.KOSMOS_HEADLESS === "1"');
    expect(sync).toContain("const refreshed = await rpc(op.getIntegrations)");
    expect(sync).toContain("normalizeIntegrationSnapshot(refreshed.data)");
    expect(main).toContain("app.requestSingleInstanceLock()");
    expect(main).toContain('app.on("second-instance"');
    expect(main).toContain("win.focus()");
    expect(main).toContain("void waitForEngineReady()");
    expect(helpers).toContain("windowsHide: true");
  });

  test("keeps browser integration login in the main process with fresh cookies and a headless guard", () => {
    const api = source("../src/manager-api.ts");
    const preload = source("preload.ts");
    const main = source("main.ts");
    const login = source("integration-login.ts");
    const credential = source("integration-login-credential.ts");
    expect(api).toContain("loginLeetCode(): Promise<ManagerResult<IntegrationsSnapshot>>");
    expect(api).toContain("loginGreatFrontend(): Promise<ManagerResult<IntegrationsSnapshot>>");
    expect(preload).toContain('loginLeetCode: () => invoke("manager.loginLeetCode")');
    expect(preload).toContain('loginGreatFrontend: () => invoke("manager.loginGreatFrontend")');
    expect(preload).not.toContain("LEETCODE_SESSION");
    expect(preload).not.toContain("csrftoken");
    expect(login).toContain('process.env.KOSMOS_HEADLESS === "1"');
    expect(login).toContain('clearStorageData({ storages: ["cookies"] })');
    expect(login).toContain("https://leetcode.com/accounts/login/");
    expect(login).toContain("https://www.greatfrontend.com/profile/progress");
    expect(login).toContain('url: "https://leetcode.com/"');
    expect(login).toContain("op.setIntegrationCredential");
    expect(login).not.toContain("console.");
    expect(credential).toMatch(/closeWindow\(\);\r?\n\s+return await flow\.persistCredential\(credential\)/);
    expect(main).toContain("registerIntegrationLoginHandlers(() => managerWindow)");
  });

  test("normalizes Engine settings and accepts only a boolean Usage Tracker patch", () => {
    expect(
      normalizeEngineSettings({
        desktop_host: { warm_timeout_seconds: 0, future: "keep-private" },
        usage_tracker: { enabled: false, secret: "drop" },
        future_root: { x: true },
      }),
    ).toEqual({
      desktop_host: { warm_timeout_seconds: 0 },
      usage_tracker: { enabled: false },
    });
    expect(normalizeEngineSettings({}).usage_tracker.enabled).toBe(true);
    expect(validateUsageTrackerSettingsPatch({ enabled: true })).toBe(true);
    expect(validateUsageTrackerSettingsPatch({ enabled: false })).toBe(true);
    expect(validateUsageTrackerSettingsPatch({ enabled: true, extra: false })).toBe(false);
    expect(validateUsageTrackerSettingsPatch({ enabled: "yes" })).toBe(false);
    expect(validateUsageTrackerSettingsPatch(undefined)).toBe(false);
    const api = source("../src/manager-api.ts");
    const preload = source("preload.ts");
    const main = source("main.ts");
    expect(api).toContain("setUsageTracker");
    expect(preload).toContain("manager.setUsageTracker");
    expect(main).toContain("validateUsageTrackerSettingsPatch");
  });

  test("normalizes Package v1 replies and rejects privileged install inputs", () => {
    expect(validPackageId("com.kosmos.demo")).toBe(true);
    expect(validPackageId("../outside")).toBe(false);
    expect(validPackageVersion("1.2.3")).toBe(true);
    expect(validPackageVersion("latest")).toBe(false);
    expect(
      normalizePackageSnapshot({
        packages: [
          {
            id: "com.kosmos.demo",
            icon_path: "C:\\Kosmos\\demo.ico",
            version: "1.2.3",
            kind: "app",
            enabled: true,
            worker_state: "running",
            publisher: "Kosmos",
            secret: "drop",
          },
          { id: "../outside", version: "1.0.0", kind: "app" },
        ],
        catalog: [
          {
            id: "com.kosmos.demo",
            version: "1.2.3",
            kind: "app",
            archive_path: "C:\\secret",
          },
          {
            id: "com.kosmos.other",
            version: "2.0.0",
            kind: "source",
            name: "Other",
          },
        ],
        total: 3,
        truncated: false,
      }),
    ).toEqual({
      packages: [
        {
          id: "com.kosmos.demo",
          name: "com.kosmos.demo",
          icon_path: "C:\\Kosmos\\demo.ico",
          version: "1.2.3",
          kind: "app",
          publisher: "Kosmos",
          enabled: true,
          revoked: false,
          revocation_reason: null,
          worker_state: "running",
          worker_health: "running",
          update_version: null,
          catalog: false,
        },
      ],
      catalog: [
        {
          id: "com.kosmos.other",
          name: "Other",
          icon_path: null,
          version: "2.0.0",
          kind: "source",
          publisher: "Kosmos",
          enabled: false,
          revoked: false,
          revocation_reason: null,
          worker_state: "catalog",
          worker_health: "unknown",
          update_version: null,
          catalog: true,
        },
      ],
      total: 3,
      truncated: false,
    });
    const main = source("main.ts");
    expect(main).toContain("validPackageVersion");
    expect(main).toContain("Object.keys(value).length === 2");
    expect(main).toContain("Object.keys(input.config ?? {}).length === 4");
    expect(main).not.toContain("archive_path");
    expect(main).not.toContain("catalog_apply");
    expect(main).not.toContain("revocation_apply");
  });

  test("returns only safe trust state", () => {
    expect(
      normalizePackageTrustStatus({
        trust: { configured: true, revoked_packages: 1, catalog_sequence: 4 },
        catalog: {
          sequence: 4,
          expires_at: "2099-01-01T00:00:00Z",
          signature: "drop",
        },
        signing_key: "drop",
      }),
    ).toMatchObject({
      state: "usable",
      configured: true,
      revoked_packages: 1,
      catalog_sequence: 4,
      expires_at: "2099-01-01T00:00:00Z",
    });
    expect(normalizePackageTrustStatus({ trust: { configured: false } }).state).toBe("unavailable");
    expect(
      normalizePackageTrustStatus({
        trust: { configured: true, fault_code: "expired" },
      }).state,
    ).toBe("stale");
  });
  test("keeps File Index on the bounded Engine v1 allowlist", () => {
    expect(validateFileIndexSettingsPatch({ enabled: true, include_hidden: false })).toEqual({
      enabled: true,
      include_hidden: false,
    });
    expect(validateFileIndexSettingsPatch({ enabled: true, search: true })).toBeNull();
    expect(validateFileIndexSettingsPatch({ enabled: "yes" })).toBeNull();
    expect(validateFileIndexPath("C:\\fixture\\root")).toBe(true);
    expect(validateFileIndexPath("\\\\server\\share")).toBe(false);
    expect(validateFileIndexPath("//server/share")).toBe(false);
    expect(validateFileIndexPath(42)).toBe(false);
    expect(
      normalizeFileIndexSettings({
        enabled: true,
        roots: [`C:\\fixture\\root`, "x".repeat(5000)],
        ignore_patterns: ["node_modules", { bad: true }],
        ntfs_status: "active",
        scan_progress: {
          phase: "scan",
          root: "C:\\fixture\\root",
          files_indexed: 3,
        },
        secret: "drop",
      }),
    ).toMatchObject({
      enabled: true,
      roots: ["C:\\fixture\\root"],
      ignore_patterns: ["node_modules"],
      ntfs_status: "active",
      scan_progress: { phase: "scan", files_indexed: 3 },
    });
    expect(
      normalizeFileIndexDiagnostics({
        db_size_bytes: -1,
        files_count: 4,
        roots: ["C:\\fixture\\root"],
        risk_level: "danger",
        risk_reasons: ["C:\\secret\\user-file.txt"],
      }),
    ).toMatchObject({
      files_count: 4,
      db_size_bytes: 0,
      risk_level: "danger",
      risk_reasons: ["Индекс содержит потенциально слишком широкий корень или большой объём."],
    });
    const api = source("../src/manager-api.ts");
    const preload = source("preload.ts");
    const main = source("main.ts");
    for (const method of [
      "getFileIndexSettings",
      "setFileIndexSettings",
      "getFileIndexDiagnostics",
      "addFileIndexRoot",
      "removeFileIndexRoot",
      "addFileIndexIgnore",
      "removeFileIndexIgnore",
      "rescanFileIndex",
      "clearFileIndexCache",
      "pickFileIndexRoot",
    ]) {
      expect(api).toContain(method);
      expect(preload).toContain(method);
    }
    for (const operation of [
      "file_index.settings_get",
      "file_index.settings_set",
      "file_index.diagnostics",
      "file_index.scope_add",
      "file_index.scope_remove",
      "file_index.ignore_add",
      "file_index.ignore_remove",
      "file_index.rescan",
      "file_index.clear_cache",
    ])
      expect(api).toContain(operation);
    expect(main).toContain('process.env.KOSMOS_HEADLESS === "1"');
    expect(main).toContain("showOpenDialog");
    expect(main).toContain("senderWindow !== managerWindow");
    expect(main).toContain("Недопустимый источник выбора папки.");
    expect(api).not.toContain('"file_index.search"');
    expect(api).not.toContain('"file_index.open"');
  });
  test("uses Engine API v1 on health and RPC requests", () => {
    const client = source("engine-client.ts");
    expect(client).toContain('"X-Kosmos-Api-Version": ENGINE_API_VERSION');
    expect(client).not.toContain("kepler.lock.json");
  });

  test("keeps Manager IPC on typed v1 operation contracts", () => {
    const api = source("../src/manager-api.ts");
    const main = source("main.ts");
    expect(api).toMatch(/listObjectTypes:\s*"manager\.data\.types"/);
    expect(api).toMatch(/listObjects:\s*"manager\.data\.list"/);
    expect(api).toMatch(/searchObjects:\s*"manager\.data\.search"/);
    expect(api).toMatch(/getPairingCode:\s*"get_own_iroh_ticket"/);
    expect(main).toContain("toPairingParams");
    expect(main).toContain("toDeviceParams");
    expect(main).toContain("normalizeSyncSnapshot");
    expect(main).toContain('message: "Не удалось проверить активное состояние."');
    expect(main).toContain('message: "Не удалось проверить системную службу."');
    expect(main).toContain('error: "Служба не выполнила действие."');
    for (const file of ["main.ts", "manager-contract.ts"]) {
      const focusSource = source(file);
      for (const signature of ["Рќ", "РЎ", "Р°", "Рµ", "СЃ", "СЂ", "�"])
        expect(focusSource).not.toContain(signature);
    }
  });

  test("normalizes sync wire data and never forwards raw peer fields", () => {
    expect(toPairingParams("  ticket  ")).toEqual({ pairing_code: "ticket" });
    expect(toDeviceParams("  device-a  ")).toEqual({ device_id: "device-a" });
    expect(
      normalizeSyncSnapshot({
        running: true,
        pairing_available: true,
        own_pairing_code_available: true,
        transport: "untrusted-wire-value",
        local_device: {
          device_id: "local",
          device_name: "Ноутбук",
          secret: "drop",
        },
        peers: [
          {
            device_id: "peer",
            device_name: "Телефон",
            status: "online",
            last_seen: "2026-08-01T12:34:56+03:00",
            raw_ticket: "drop",
          },
          { device_id: "numeric-date", last_seen: "1" },
          { device_id: "bad-date", last_seen: "2026-02-30T12:00:00Z" },
          { malformed: true },
        ],
      }),
    ).toEqual({
      running: true,
      status: "running",
      transport: "unknown",
      local_device: { id: "local", name: "Ноутбук" },
      peers: [
        {
          id: "peer",
          name: "Телефон",
          status: "online",
          last_seen: "2026-08-01T09:34:56.000Z",
        },
        {
          id: "numeric-date",
          name: "numeric-date",
          status: "unknown",
          last_seen: null,
        },
        {
          id: "bad-date",
          name: "bad-date",
          status: "unknown",
          last_seen: null,
        },
      ],
      pairing_available: true,
      own_pairing_code_available: true,
    });
    expect(
      normalizeSyncSnapshot({
        local_device: {
          device_id: `  ${"d".repeat(256)}  `,
          device_name: ` ${"n".repeat(256)} `,
          secret: "drop",
        },
      }).local_device,
    ).toEqual({ id: "d".repeat(256), name: "n".repeat(256) });
    expect(
      normalizeSyncSnapshot({
        local_device: {
          device_id: "d".repeat(257),
          device_name: "n".repeat(257),
        },
      }).local_device,
    ).toBeNull();
    expect(
      normalizeSyncSnapshot({
        local_device: { id: "  fallback  ", name: ` ${"n".repeat(257)} ` },
      }).local_device,
    ).toEqual({ id: "fallback", name: "fallback" });
  });

  test("bounds pairing input and trims only a valid ticket", () => {
    expect(validPairingCode("  12345678  ")).toBe(true);
    expect(validPairingCode("1234567")).toBe(false);
    expect(validPairingCode("x".repeat(257))).toBe(false);
    expect(normalizePairingCode("  fixture-ticket  ")).toBe("fixture-ticket");
    expect(normalizePairingCode(" ")).toBeNull();
    expect(normalizePairingCode("x".repeat(4097))).toBeNull();
    const main = source("main.ts");
    expect(main).toContain("validPairingCode");
    expect(main).toContain("normalizePairingCode");
  });

  test("window lifecycle never owns or terminates Engine", () => {
    const main = source("main.ts");
    expect(main).toContain("show: false");
    expect(main).toContain('win.once("ready-to-show"');
    expect(main).toContain('"did-fail-load"');
    expect(main).toContain("if (!isMainFrame) return");
    expect(main).toContain("presentManagerWindow(managerWindow)");
    expect(main).toContain("if (win.isMinimized()) win.restore()");
    expect(main).toContain('win.on("closed"');
    expect(main).toContain('win.webContents.on("render-process-gone"');
    expect(main).not.toContain("process.kill(");
    expect(main).toContain("app.quit()");
    expect(main).toContain("KOSMOS_HEADLESS");
    expect(main).toContain("KOSMOS_TEST_MODE");
  });

  test("every Manager child process stays console-free on Windows", () => {
    const helpers = source("main-helpers.ts");
    expect(helpers.match(/\bspawn\(/g)).toHaveLength(4);
    expect(helpers.match(/windowsHide: true/g)).toHaveLength(4);
  });

  test("About version is a fixed read-only app capability", () => {
    const api = source("../src/manager-api.ts");
    const preload = source("preload.ts");
    const main = source("main.ts");
    expect(api).toContain("getAppVersion(): Promise<ManagerResult<string>>");
    expect(preload).toContain('getAppVersion: () => invoke("manager.getAppVersion")');
    expect(main).toContain('ipcMain.handle("manager.getAppVersion"');
    expect(main).toContain("app.getVersion()");
    expect(preload).not.toContain("ipcRenderer.send");
  });

  test("renderer receives only the typed capability surface", () => {
    const preload = source("preload.ts");
    expect(preload).toContain('contextBridge.exposeInMainWorld("kosmosManager"');
    expect(preload).not.toContain("auth_token");
    expect(preload).not.toContain("lock.json");
    expect(preload).not.toContain("fetch(");
    const sync = source("../src/views/SyncView.vue");
    expect(sync).not.toContain("window.kepler");
    expect(sync).not.toMatch(/\b(?:SELECT|INSERT INTO|UPDATE \w+|DELETE FROM)\b/i);
    expect(sync).toContain("clearInterval(timer)");
    expect(sync).toContain("if (refreshing.value) return");
    expect(sync).toContain("const SYNC_REFRESH_INTERVAL_MS = 8_000");
    expect(sync).toContain("setInterval(() => void refresh(), SYNC_REFRESH_INTERVAL_MS)");
  });

  test("support bundle cancellation is a separate Engine operation", () => {
    const main = source("main.ts");
    expect(main).toContain("op.cancelSupportBundle");
    expect(main).toContain("destination: picked.filePath");
    expect(main).toContain('defaultPath: "kosmos-support.json"');
    expect(main).toContain('extensions: ["json"]');
    expect(main).not.toContain("op.saveSupportBundle, { handle, cancelled");
  });

  test("diagnostics filesystem surface is exactly typed and path-free", () => {
    const api = source("../src/manager-api.ts");
    const preload = source("preload.ts");
    const main = source("main.ts");
    for (const method of [
      "listCrashReports",
      "clearCrashReports",
      "openCrashReportsFolder",
      "openLogsFolder",
    ]) {
      expect(api).toContain(`${method}()`);
      expect(preload).toContain(`${method}: () => invoke("manager.${method}")`);
      expect(main).toContain(`ipcMain.handle("manager.${method}"`);
    }
    expect(source("diagnostics-files.ts")).toContain("lstat");
    expect(source("diagnostics-files.ts")).not.toContain("readFile");
  });

  test("dictation bridge allowlists, validates, and redacts Runtime wire data", () => {
    const api = source("../src/manager-api.ts");
    const main = source("main.ts");
    const preload = source("preload.ts");
    for (const operation of [
      "dictation.get_config",
      "dictation.update_config",
      "dictation.get_stats",
      "dictation.verify_api_key",
      "dictation.set_api_key",
      "dictation.clear_api_key",
      "dictation.begin_hotkey_capture",
      "dictation.end_hotkey_capture",
      "dictation.test_connectivity",
      "dictation.list_local_models",
      "dictation.download_local_model",
      "dictation.use_local_model",
      "dictation.delete_local_model",
      "dictation.list_pending",
      "dictation.retry",
      "dictation.discard",
      "dictation.retry_all",
      "dictation.discard_all",
    ])
      expect(api).toContain(operation);
    expect(validateDictationConfigPatch({ injectMode: "clipboard_only" })).toEqual({
      injectMode: "clipboard_only",
    });
    expect(validateDictationConfigPatch({ injectMode: "unsafe" })).toBeNull();
    expect(validateDictationConfigPatch({ key: "not-config" })).toBeNull();
    expect(
      normalizeDictationConfig({
        config: { injectMode: "clipboard_only", apiKey: "never-forward" },
        hasApiKey: true,
      }),
    ).toMatchObject({
      config: { injectMode: "clipboard_only" },
      hasApiKey: true,
    });
    expect(
      normalizeDictationStats({
        totalRecordSeconds: 3,
        totalWords: 4,
        timeSavedSeconds: 5,
        token: "never-forward",
      }),
    ).toEqual({ totalSeconds: 3, totalWords: 4, savedSeconds: 5 });
    expect(
      normalizeDictationLocalModels({
        commandInstalled: true,
        models: [
          {
            id: "small",
            speedScore: 1,
            accuracyScore: 1,
            transcriptionSupported: true,
            name: "Small",
          },
        ],
      }),
    ).toEqual([
      {
        id: "small",
        name: "Small",
        description: "",
        downloaded: false,
        selected: false,
      },
    ]);
    expect(
      normalizeConnectivity({
        ok: true,
        totalMs: 42,
        firstFailure: "secret",
        stages: [{ name: "dns", ok: true, ms: 2, error: null }],
      }),
    ).toEqual({ stages: [{ name: "dns", ok: true, ms: 2, error: null }] });
    expect(
      normalizeDictationPending({
        items: [
          {
            uuid: "safe-item",
            createdAt: "2026-08-01",
            attempts: 1,
            key: "never-forward",
          },
        ],
      }),
    ).toEqual([
      {
        uuid: "safe-item",
        createdAt: "2026-08-01",
        attempts: 1,
        lastError: null,
      },
    ]);
    expect(
      normalizeDictationEvent({
        event: "dictation_local_model_download_progress",
        modelId: "small",
        percent: 101,
        token: "never-forward",
      }),
    ).toEqual({ kind: "download", modelId: "small", percent: 100 });
    expect(main).toContain("validateDictationConfigPatch");
    expect(main).toContain("normalizeDictationEvent");
    expect(preload).not.toContain("auth_token");
    expect(preload).not.toContain("window.kepler");
    expect(preload).toContain("manager:dictation-event");
    expect(preload).not.toContain("new Set");
    expect(source("engine-client.ts")).not.toContain("new Set");
    expect(main).not.toContain("new Set");
  });

  test("focus bridge keeps raw references, bounds fields, and forbids execution ops", () => {
    expect(
      normalizeFocusBlocklists({
        blocklists: [
          {
            id: "work",
            name: "Работа",
            domains: ["example.com", "@other"],
            createdAt: "2026",
            icon: "🛡️",
            kind: "raw",
            preset: true,
            secret: "drop",
          },
          { id: "", name: "bad" },
        ],
      }),
    ).toEqual([
      {
        id: "work",
        name: "Работа",
        domains: ["example.com", "@other"],
        createdAt: "2026",
        icon: "🛡️",
        kind: "raw",
        preset: true,
      },
    ]);
    expect(
      normalizeFocusActiveState({
        active: true,
        blocklist_id: "work",
        started_at: "2026",
        token: "drop",
      }),
    ).toEqual({ active: true, blocklist_id: "work", started_at: "2026" });
    expect(isActiveFocusBlocklist({ active: true, blocklist_id: "work" }, "work")).toBe(true);
    expect(isActiveFocusBlocklist({ active: false, blocklist_id: "work" }, "work")).toBe(false);
    expect(isValidFocusActiveState({ active: true, blocklist_id: "work" })).toBe(true);
    expect(isValidFocusActiveState({ active: "yes", blocklist_id: "work" })).toBe(false);
    expect(isValidFocusActiveState({ active: true, blocklist_id: "bad id" })).toBe(false);
    const api = source("../src/manager-api.ts");
    const main = source("main.ts");
    for (const operation of [
      "focus.list_blocklists",
      "focus.upsert_blocklist",
      "focus.delete_blocklist",
      "focus.get_active_state",
    ])
      expect(api).toContain(operation);
    expect(main).toContain("Нельзя изменить активный блок-лист.");
    expect(main).toContain("Нельзя удалить активный блок-лист.");
    expect(main).not.toContain("focus.set_active_state");
    expect(main).not.toContain("focus.resolve_blocklist_domains");
    expect(main).not.toContain("stderr");
  });
});
