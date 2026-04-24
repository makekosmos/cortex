export type {
  AppSettings,
  DailyPlaytime,
  DiskSpeedResult,
  ExeEntry,
  GamePlaytime,
  PlaytimeStats,
  ProcessEntry,
  SystemCpuInfo,
  SystemDiskInfo,
  SystemGpuInfo,
  SystemInfo,
  SystemMemoryInfo,
  SystemMonitorInfo,
} from "../../../src/types";

export interface SettingsRepository {
  listSettings(): Promise<Record<string, string>>;
  upsertSettings(entries: Record<string, string>): Promise<void>;
  getSetting(key: string): Promise<string | null>;
  upsertSetting(key: string, value: string): Promise<void>;
  listScanDirectories(): Promise<string[]>;
  addScanDirectory(path: string): Promise<void>;
  removeScanDirectory(path: string): Promise<void>;
}

// Read-only query surface for playtime stats. Arrancador consumes Ark usage
// data only; legacy local history is imported into Ark separately.
export interface PlaytimeStatsRepository {
  getRangeStats(rangeStart: string, rangeEnd: string): Promise<{
    dailyTotals: Array<{ date: string; seconds: number }>;
    perGameTotals: Array<{ id: string; name: string; seconds: number }>;
  }>;
  getDailyTotals(rangeStart: string, rangeEnd: string): Promise<
    Array<{ date: string; seconds: number }>
  >;
  getPerGameTotals(rangeStart: string, rangeEnd: string): Promise<
    Array<{ id: string; name: string; seconds: number }>
  >;
}

export interface ScanStreamOptions {
  signal?: AbortSignal;
  onEntry?: (entry: import("../../../src/types").ExeEntry) => void | Promise<void>;
}

export interface SystemServiceOptions {
  getGpuInfo?: () => Promise<import("../../../src/types").SystemGpuInfo[]>;
  getMonitorInfo?: () => Promise<import("../../../src/types").SystemMonitorInfo[]>;
  getDiskInfo?: () => Promise<import("../../../src/types").SystemDiskInfo[]>;
}
