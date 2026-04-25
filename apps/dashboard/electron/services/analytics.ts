import fs from "node:fs";
import path from "node:path";

import { ArkClient, type ArkUsageAnalyticsSnapshot } from "@kepler/ark";

import type {
  DashboardLoadOptions,
  DashboardSnapshot,
  DatabaseStatus,
  UsageSummary,
} from "../../shared/analytics.ts";

type DashboardAnalyticsProvider = {
  snapshot(options?: {
    rangeDays?: number;
    topAppsLimit?: number;
    recentSessionsLimit?: number;
  }): Promise<ArkUsageAnalyticsSnapshot>;
};

export interface DashboardAnalyticsDeps {
  analyticsProvider?: DashboardAnalyticsProvider;
  arkCoreRpcPath?: string;
  appRoot?: string;
  resourcesPath?: string;
}

function clampPositiveInt(value: number | undefined, fallback: number): number {
  if (!Number.isFinite(value)) {
    return fallback;
  }
  return Math.max(1, Math.floor(value ?? fallback));
}

function emptySummary(): UsageSummary {
  return {
    trackedAppCount: 0,
    sessionCount: 0,
    eventCount: 0,
    totalForegroundMs: 0,
    totalIdleMs: 0,
    firstRecordedAt: null,
    lastRecordedAt: null,
  };
}

function emptySnapshot(status: DatabaseStatus): DashboardSnapshot {
  return {
    generatedAt: new Date().toISOString(),
    status,
    summary: emptySummary(),
    dailyTrend: [],
    hourlyHeatmap: [],
    topApps: [],
    recentSessions: [],
  };
}

function arkCoreRpcBinaryName() {
  return process.platform === "win32" ? "ark-core-rpc.exe" : "ark-core-rpc";
}

export function resolveArkCoreRpcBinaryPath(
  options: Pick<DashboardAnalyticsDeps, "appRoot" | "resourcesPath"> = {},
) {
  const binaryName = arkCoreRpcBinaryName();
  const packagedPath = path.join(
    options.resourcesPath ?? process.resourcesPath ?? "",
    "ark-core",
    binaryName,
  );
  if (fs.existsSync(packagedPath)) {
    return packagedPath;
  }

  const appRoot = path.resolve(options.appRoot ?? process.env.APP_ROOT ?? process.cwd());
  const repoRoot = path.basename(appRoot) === "dashboard"
    ? path.resolve(appRoot, "..", "..")
    : appRoot;
  const releasePath = path.join(
    repoRoot,
    "packages",
    "ark-core",
    "rust",
    "target",
    "release",
    binaryName,
  );
  if (fs.existsSync(releasePath)) {
    return releasePath;
  }

  return path.join(
    repoRoot,
    "packages",
    "ark-core",
    "rust",
    "target",
    "debug",
    binaryName,
  );
}

export function resolveDefaultArkDbPath(): string {
  if (process.env.ARK_DB_PATH && process.env.ARK_DB_PATH.trim().length > 0) {
    return process.env.ARK_DB_PATH;
  }

  const appData = process.env.APPDATA ?? process.env.LOCALAPPDATA;
  if (!appData) {
    return path.join(process.cwd(), "ark.db");
  }

  return path.join(appData, "Kepler", "ark.db");
}

function createDashboardArkClient(
  dbPath: string,
  deps: DashboardAnalyticsDeps,
): ArkClient {
  return new ArkClient({
    spaceId: "dashboard",
    deviceId: "dashboard-main",
    deviceName: "Dashboard",
    dbPath,
    sidecarPath: deps.arkCoreRpcPath ?? resolveArkCoreRpcBinaryPath(deps),
    requestTimeoutMs: 10_000,
  });
}

export async function loadDashboardSnapshot(
  options: DashboardLoadOptions = {},
  selectedDbPath?: string | null,
  deps: DashboardAnalyticsDeps = {},
): Promise<DashboardSnapshot> {
  const rangeDays = clampPositiveInt(options.rangeDays, 21);
  const topAppsLimit = clampPositiveInt(options.topAppsLimit, 8);
  const recentSessionsLimit = clampPositiveInt(options.recentSessionsLimit, 24);
  const explicitDbPath = options.dbPath ?? null;
  const resolvedPath = explicitDbPath ?? selectedDbPath ?? resolveDefaultArkDbPath();
  const source: DatabaseStatus["source"] = explicitDbPath
    ? "explicit"
    : selectedDbPath
      ? "selected"
      : "default";

  if (!resolvedPath || !fs.existsSync(resolvedPath)) {
    return emptySnapshot({
      path: resolvedPath,
      exists: false,
      readable: false,
      source,
      message:
        "Файл Ark DB не найден. Выберите существующую базу или дождитесь первых usage-данных.",
    });
  }

  const ownedClient = deps.analyticsProvider
    ? null
    : createDashboardArkClient(resolvedPath, deps);
  const analytics = deps.analyticsProvider ?? ownedClient?.usage.analytics;

  try {
    const snapshot = await analytics?.snapshot({
      rangeDays,
      topAppsLimit,
      recentSessionsLimit,
    });
    if (!snapshot) {
      throw new Error("Ark analytics provider is not available");
    }

    return {
      ...snapshot,
      status: {
        path: resolvedPath,
        exists: true,
        readable: true,
        source,
        message:
          snapshot.summary.sessionCount > 0
            ? null
            : "База читается, но usage-сессии пока не записаны.",
      },
    };
  } catch (error) {
    return emptySnapshot({
      path: resolvedPath,
      exists: true,
      readable: false,
      source,
      message: `Не удалось прочитать Ark usage analytics: ${(error as Error).message}`,
    });
  } finally {
    await ownedClient?.stop();
  }
}
