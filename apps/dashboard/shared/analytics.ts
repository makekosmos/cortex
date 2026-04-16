export interface DashboardLoadOptions {
  dbPath?: string | null;
  rangeDays?: number;
  topAppsLimit?: number;
  recentSessionsLimit?: number;
}

export type DashboardPlatform = "win32" | "darwin" | "linux";

export interface DatabaseStatus {
  path: string | null;
  exists: boolean;
  readable: boolean;
  source: "default" | "selected" | "explicit";
  message: string | null;
}

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

export interface DashboardSnapshot {
  generatedAt: string;
  status: DatabaseStatus;
  summary: UsageSummary;
  dailyTrend: DailyTrendPoint[];
  hourlyHeatmap: HourlyHeatmapCell[];
  topApps: TopAppEntry[];
  recentSessions: RecentSessionEntry[];
}
