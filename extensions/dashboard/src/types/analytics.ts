// Shape of `get_usage_analytics` operation в ARK runtime. Совпадает с
// ArkUsageAnalyticsSnapshot из @kepler/ark — продублирован локально, чтобы
// extension не зависел от типов SDK при сборке.

export interface UsageSummary {
  trackedAppCount: number;
  sessionCount: number;
  eventCount: number;
  totalForegroundMs: number;
  totalIdleMs: number;
  firstRecordedAt: string | null;
  lastRecordedAt: string | null;
}

export interface DailyTrendPoint {
  date: string;
  foregroundMs: number;
  idleMs: number;
  sessions: number;
}

export interface HourlyHeatmapCell {
  weekday: number;
  hour: number;
  foregroundMs: number;
}

export interface TopAppEntry {
  id: string;
  displayName: string;
  processName: string;
  normalizedPath: string;
  foregroundMs: number;
  idleMs: number;
  sessions: number;
  lastSeenAt: string | null;
}

export interface RecentSessionEntry {
  id: string;
  trackedAppId: string;
  displayName: string;
  processName: string;
  platform: string;
  deviceName: string;
  startedAt: string;
  endedAt: string | null;
  foregroundMs: number;
  idleMs: number;
  windowTitle: string | null;
}

export interface UsageAnalyticsSnapshot {
  generatedAt: string;
  summary: UsageSummary;
  dailyTrend: DailyTrendPoint[];
  hourlyHeatmap: HourlyHeatmapCell[];
  topApps: TopAppEntry[];
  recentSessions: RecentSessionEntry[];
}

export interface UsageAnalyticsOptions {
  rangeDays?: number;
  topAppsLimit?: number;
  recentSessionsLimit?: number;
}
