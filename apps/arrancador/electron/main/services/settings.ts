import type { AppSettings } from "./contracts";
import type { SettingsRepository } from "./contracts";
import {
  getDefaultAppSettings,
  normalizeAppSettings,
  serializeAppSettings,
  shouldStartMinimizedInTray,
} from "./helpers/settings";

export interface SettingsService {
  getAllSettings(): Promise<AppSettings>;
  updateSettings(settings: AppSettings): Promise<void>;
  getSetting(key: string): Promise<string | null>;
  setSetting(key: string, value: string): Promise<void>;
  addScanDirectory(path: string): Promise<void>;
  getScanDirectories(): Promise<string[]>;
  removeScanDirectory(path: string): Promise<void>;
  shouldStartMinimizedInTray(settings?: AppSettings | null): boolean;
}

function mergeSettings(raw: Record<string, string>) {
  return normalizeAppSettings({
    ...getDefaultAppSettings(),
    ...raw,
  });
}

export function createSettingsService(
  repository: SettingsRepository,
): SettingsService {
  return {
    async getAllSettings() {
      return mergeSettings(await repository.listSettings());
    },
    async updateSettings(settings: AppSettings) {
      await repository.upsertSettings(serializeAppSettings(normalizeAppSettings(settings)));
    },
    getSetting: (key: string) => repository.getSetting(key),
    setSetting: (key: string, value: string) => repository.upsertSetting(key, value),
    addScanDirectory: (path: string) => repository.addScanDirectory(path),
    getScanDirectories: () => repository.listScanDirectories(),
    removeScanDirectory: (path: string) => repository.removeScanDirectory(path),
    shouldStartMinimizedInTray,
  };
}
