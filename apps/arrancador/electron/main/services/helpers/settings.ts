import type { AppSettings } from "../contracts";

const DEFAULT_SETTINGS: AppSettings = {
  theme: "system",
  ludusavi_path: "",
  backup_directory: "",
  auto_backup: true,
  backup_before_launch: false,
  backup_compression_enabled: true,
  backup_compression_level: 60,
  backup_skip_compression_once: false,
  max_backups_per_game: 5,
  rawg_api_key: "",
  start_minimized_in_tray: false,
};

function clamp(value: number, min: number, max: number) {
  if (Number.isNaN(value)) {
    return min;
  }
  return Math.min(max, Math.max(min, value));
}

function parseBoolean(value: unknown, fallback: boolean) {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "number") {
    return value !== 0;
  }
  if (typeof value === "string") {
    return ["true", "1", "yes", "on"].includes(value.trim().toLowerCase());
  }
  return fallback;
}

function parseIntValue(value: unknown, fallback: number, min: number, max: number) {
  if (typeof value === "number" && Number.isFinite(value)) {
    return clamp(Math.trunc(value), min, max);
  }
  if (typeof value === "string" && value.trim()) {
    const parsed = Number.parseInt(value, 10);
    if (!Number.isNaN(parsed)) {
      return clamp(parsed, min, max);
    }
  }
  return fallback;
}

export function getDefaultAppSettings(): AppSettings {
  return { ...DEFAULT_SETTINGS };
}

export function normalizeAppSettings(
  input: Partial<Record<keyof AppSettings, unknown>> &
    Record<string, unknown> = {},
): AppSettings {
  return {
    theme: typeof input.theme === "string" && input.theme.trim()
      ? input.theme
      : DEFAULT_SETTINGS.theme,
    ludusavi_path:
      typeof input.ludusavi_path === "string" ? input.ludusavi_path : "",
    backup_directory:
      typeof input.backup_directory === "string" ? input.backup_directory : "",
    auto_backup: parseBoolean(input.auto_backup, DEFAULT_SETTINGS.auto_backup),
    backup_before_launch: parseBoolean(
      input.backup_before_launch,
      DEFAULT_SETTINGS.backup_before_launch,
    ),
    backup_compression_enabled: parseBoolean(
      input.backup_compression_enabled,
      DEFAULT_SETTINGS.backup_compression_enabled,
    ),
    backup_compression_level: parseIntValue(
      input.backup_compression_level,
      DEFAULT_SETTINGS.backup_compression_level,
      1,
      100,
    ),
    backup_skip_compression_once: parseBoolean(
      input.backup_skip_compression_once,
      DEFAULT_SETTINGS.backup_skip_compression_once,
    ),
    max_backups_per_game: parseIntValue(
      input.max_backups_per_game,
      DEFAULT_SETTINGS.max_backups_per_game,
      1,
      100,
    ),
    rawg_api_key:
      typeof input.rawg_api_key === "string" ? input.rawg_api_key : "",
    start_minimized_in_tray: parseBoolean(
      input.start_minimized_in_tray,
      DEFAULT_SETTINGS.start_minimized_in_tray,
    ),
  };
}

export function serializeAppSettings(settings: AppSettings): Record<string, string> {
  return {
    theme: settings.theme,
    ludusavi_path: settings.ludusavi_path,
    backup_directory: settings.backup_directory,
    auto_backup: String(settings.auto_backup),
    backup_before_launch: String(settings.backup_before_launch),
    backup_compression_enabled: String(settings.backup_compression_enabled),
    backup_compression_level: String(
      clamp(settings.backup_compression_level, 1, 100),
    ),
    backup_skip_compression_once: String(settings.backup_skip_compression_once),
    max_backups_per_game: String(clamp(settings.max_backups_per_game, 1, 100)),
    rawg_api_key: settings.rawg_api_key,
    start_minimized_in_tray: String(settings.start_minimized_in_tray),
  };
}

export function shouldStartMinimizedInTray(settings?: AppSettings | null) {
  return settings?.start_minimized_in_tray ?? DEFAULT_SETTINGS.start_minimized_in_tray;
}
