import type { JsonValue } from "./ark-client.types.js";

export interface ArkTrackedAppRecord {
  id: string;
  platform: string;
  exePath: string;
  normalizedExePath: string;
  processName: string;
  displayName: string | null;
  publisher: string | null;
  iconRef: string | null;
  firstSeenAt: string;
  lastSeenAt: string;
}

export interface ArkUsageSessionRecord {
  id: string;
  trackedAppId: string;
  deviceId: string;
  deviceName: string;
  platform: string;
  startedAt: string;
  endedAt: string | null;
  runtimeMs: number;
  foregroundMs: number;
  idleMs: number;
  windowTitle: string | null;
  processName: string;
  exePath: string;
  pidStart: number | null;
  pidEnd: number | null;
  metaJson: JsonValue;
}

export interface ArkUsageEventRecord {
  id: string;
  trackedAppId: string;
  usageSessionId: string | null;
  deviceId: string;
  deviceName: string;
  platform: string;
  occurredAt: string;
  kind: string;
  windowTitle: string | null;
  processName: string;
  exePath: string;
  pid: number | null;
  isForeground: boolean;
  isIdle: boolean;
  metaJson: JsonValue;
}

export interface ArkUsageSnapshot {
  trackedApps: ArkTrackedAppRecord[];
  usageSessions: ArkUsageSessionRecord[];
  usageEvents: ArkUsageEventRecord[];
}

export interface ArkUsageAnalyticsOptions {
  rangeDays?: number;
  topAppsLimit?: number;
  recentSessionsLimit?: number;
}

export interface ArkUsageSummary {
  trackedAppCount: number;
  sessionCount: number;
  eventCount: number;
  totalRuntimeMs: number;
  totalForegroundMs: number;
  totalIdleMs: number;
  firstRecordedAt: string | null;
  lastRecordedAt: string | null;
}

export interface ArkDailyTrendPoint {
  date: string;
  foregroundMs: number;
  idleMs: number;
  sessions: number;
}

export interface ArkHourlyHeatmapCell {
  weekday: number;
  hour: number;
  foregroundMs: number;
}

export interface ArkTopAppEntry {
  id: string;
  displayName: string;
  processName: string;
  normalizedPath: string;
  iconRef: string | null;
  runtimeMs: number;
  foregroundMs: number;
  idleMs: number;
  sessions: number;
  lastSeenAt: string | null;
}

export interface ArkRecentSessionEntry {
  id: string;
  trackedAppId: string;
  displayName: string;
  processName: string;
  platform: string;
  deviceName: string;
  startedAt: string;
  endedAt: string | null;
  runtimeMs: number;
  foregroundMs: number;
  idleMs: number;
  windowTitle: string | null;
}

export interface ArkUsageProcessCandidate {
  trackedAppId: string;
  displayName: string;
  exePath: string | null;
  processName: string | null;
  lastSeenAt: string | null;
  sessionCount: number;
  bindingMatchType: "exe_path" | "process_name";
  bindingMatchValue: string;
  bindingNormalizedValue: string;
}

export interface ArkUsageGamePlaytimeBinding {
  gameId: string;
  gameName: string;
  matchType: "exe_path" | "process_name";
  matchValue: string;
}

export interface ArkUsageGamePlaytimeAggregate {
  gameId: string;
  gameName: string;
  totalSeconds: number;
  sessionCount: number;
  lastPlayed: string | null;
}

export interface ArkUsageGameDailyTotal {
  date: string;
  seconds: number;
}

export interface ArkUsageGameRangeTotal {
  gameId: string;
  gameName: string;
  seconds: number;
}

export interface ArkUsageGamePlaytimeSummaryOptions {
  bindings: ArkUsageGamePlaytimeBinding[];
  rangeStart?: string;
  rangeEnd?: string;
}

export interface ArkUsageGamePlaytimeSummary {
  aggregates: ArkUsageGamePlaytimeAggregate[];
  dailyTotals: ArkUsageGameDailyTotal[];
  perGameTotals: ArkUsageGameRangeTotal[];
}

export interface ArkUsageAnalyticsSnapshot {
  generatedAt: string;
  summary: ArkUsageSummary;
  dailyTrend: ArkDailyTrendPoint[];
  hourlyHeatmap: ArkHourlyHeatmapCell[];
  topApps: ArkTopAppEntry[];
  recentSessions: ArkRecentSessionEntry[];
}

export interface ArkUsageEntityApi<T> {
  upsert(record: T): Promise<void>;
  delete(id: string): Promise<void>;
}

export interface ArkUsageApi {
  loadAll(): Promise<ArkUsageSnapshot>;
  analytics: {
    snapshot(options?: ArkUsageAnalyticsOptions): Promise<ArkUsageAnalyticsSnapshot>;
  };
  processes: {
    recent(limit?: number): Promise<ArkUsageProcessCandidate[]>;
    search(query: string, limit?: number): Promise<ArkUsageProcessCandidate[]>;
  };
  gamePlaytime: {
    summary(options: ArkUsageGamePlaytimeSummaryOptions): Promise<ArkUsageGamePlaytimeSummary>;
  };
  trackedApps: ArkUsageEntityApi<ArkTrackedAppRecord>;
  sessions: ArkUsageEntityApi<ArkUsageSessionRecord>;
  events: ArkUsageEntityApi<ArkUsageEventRecord>;
}

export interface ArkLoadAllData {
  trackedApps?: ArkTrackedAppRecord[];
  usageSessions?: ArkUsageSessionRecord[];
  usageEvents?: ArkUsageEventRecord[];
}
