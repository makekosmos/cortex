import { invoke } from "./ipc";
import type {
  AppSettings,
  Backup,
  BackupInfo,
  DiskSpeedResult,
  NotificationItem,
  Game,
  Achievement,
  CatalogueItem,
  CatalogueSyncResult,
  NewGame,
  PlaytimeStats,
  ProcessEntry,
  RawgGame,
  RawgGameDetails,
  RestoreCheck,
  SavePathLookup,
  SystemInfo,
  UpdateGame,
} from "@/types";

// Game API
export const gamesApi = {
  getAll: () => invoke<Game[]>("get_all_games"),
  get: (id: string) => invoke<Game | null>("get_game", { id }),
  add: (game: NewGame) => invoke<Game>("add_game", { game }),
  addBatch: (games: NewGame[]) => invoke<Game[]>("add_games_batch", { games }),
  update: (update: UpdateGame) => invoke<Game>("update_game", { update }),
  delete: (id: string) => invoke<void>("delete_game", { id }),
  toggleFavorite: (id: string) => invoke<Game>("toggle_favorite", { id }),
  getFavorites: () => invoke<Game[]>("get_favorites"),
  recordLaunch: (id: string) => invoke<Game>("record_game_launch", { id }),
  search: (query: string) => invoke<Game[]>("search_games", { query }),
  existsByPath: (exePath: string) =>
    invoke<boolean>("game_exists_by_path", { exePath }),
  isInstalled: (id: string) => invoke<boolean>("is_game_installed", { id }),
  launch: (id: string) => invoke<void>("launch_game", { id }),
  getRunningInstances: (id: string) =>
    invoke<number>("get_running_instances", { id }),
  killProcesses: (id: string) => invoke<number>("kill_game_processes", { id }),
  resolveShortcutTarget: (path: string) =>
    invoke<string>("resolve_shortcut_target", { path }),
};

// Metadata API (RAWG)
export const metadataApi = {
  search: (query: string) => invoke<RawgGame[]>("search_rawg", { query }),
  getDetails: (rawgId: number) =>
    invoke<RawgGameDetails>("get_rawg_game_details", { rawgId }),
  apply: (gameId: string, rawgId: number, rename: boolean) =>
    invoke<Game>("apply_rawg_metadata", { gameId, rawgId, rename }),
  setApiKey: (key: string) => invoke<void>("set_rawg_api_key", { key }),
  getApiKey: () => invoke<string>("get_rawg_api_key"),
};

export const achievementsApi = {
  getAll: (unlockedOnly: boolean) =>
    invoke<Achievement[]>("get_all_achievements", { unlockedOnly }),
  seedDefaults: () =>
    invoke<Achievement[]>("seed_default_achievements"),
  recordEvent: (eventTrigger: string, eventContext?: string) =>
    invoke<Achievement[]>("record_achievement_event", {
      eventTrigger,
      eventContext,
    }),
};

// Backup API
export const backupApi = {
  checkLudusaviInstalled: () => invoke<boolean>("check_ludusavi_installed"),
  getLudusaviPath: () => invoke<string | null>("get_ludusavi_executable_path"),
  setLudusaviPath: (path: string) =>
    invoke<void>("set_ludusavi_path", { path }),
  setBackupDirectory: (path: string) =>
    invoke<void>("set_backup_directory", { path }),
  getBackupDirectory: () => invoke<string>("get_backup_directory_setting"),
  refreshSqobaManifest: () => invoke<void>("refresh_sqoba_manifest"),
  findGameSavePaths: (gameName: string, gameId?: string) =>
    invoke<SavePathLookup>("find_game_save_paths", { gameName, gameId }),
  findGameSaves: (gameName: string, gameId?: string) =>
    invoke<BackupInfo | null>("find_game_saves", { gameName, gameId }),
  create: (gameId: string, gameName: string, isAuto: boolean, notes?: string) =>
    invoke<Backup>("create_backup", { gameId, gameName, isAuto, notes }),
  getForGame: (gameId: string) =>
    invoke<Backup[]>("get_game_backups", { gameId }),
  restore: (backupId: string) => invoke<void>("restore_backup", { backupId }),
  delete: (backupId: string) => invoke<void>("delete_backup", { backupId }),
  shouldBackupBeforeLaunch: (gameId: string) =>
    invoke<boolean>("should_backup_before_launch", { gameId }),
  checkBackupNeeded: (gameId: string, gameName: string) =>
    invoke<boolean>("check_backup_needed", { gameId, gameName }),
  checkRestoreNeeded: (gameId: string, gameName: string) =>
    invoke<RestoreCheck>("check_restore_needed", { gameId, gameName }),
  getSettings: () => invoke<Record<string, string>>("get_backup_settings"),
  updateSettings: (settings: Record<string, string>) =>
    invoke<void>("update_backup_settings", { settings }),
};

// Settings API
export const settingsApi = {
  getAll: () => invoke<AppSettings>("get_all_settings"),
  update: (settings: AppSettings) =>
    invoke<void>("update_settings", { settings }),
  get: (key: string) => invoke<string | null>("get_setting", { key }),
  set: (key: string, value: string) =>
    invoke<void>("set_setting", { key, value }),
  addScanDirectory: (path: string) =>
    invoke<void>("add_scan_directory", { path }),
  getScanDirectories: () => invoke<string[]>("get_scan_directories"),
  removeScanDirectory: (path: string) =>
    invoke<void>("remove_scan_directory", { path }),
};

export const statsApi = {
  getPlaytimeStats: (start?: string, end?: string) =>
    invoke<PlaytimeStats>("get_playtime_stats", { start, end }),
};

export const scanApi = {
  getRunningProcesses: () => invoke<ProcessEntry[]>("get_running_processes"),
};

export const systemApi = {
  getInfo: () => invoke<SystemInfo>("get_system_info"),
  testDiskSpeed: (mountPoint: string) =>
    invoke<DiskSpeedResult>("test_disk_speed", { mountPoint }),
};

export const notificationsApi = {
  list: (unreadOnly: boolean = false) =>
    invoke<NotificationItem[]>("list_notifications", { unread_only: unreadOnly }),
  create: (level: string, title: string, message: string, source?: string) =>
    invoke<NotificationItem>("create_notification", {
      level,
      title,
      message,
      source,
    }),
  markRead: (id: string) => invoke<boolean>("mark_notification_read", { id }),
  markAllRead: () => invoke<number>("mark_all_notifications_read"),
  clear: () => invoke<number>("clear_notifications"),
};

export const catalogueApi = {
  getItems: (source?: string) =>
    invoke<CatalogueItem[]>("get_catalogue_items", { source: source ?? null }),
  search: (query: string, source?: string) =>
    invoke<CatalogueItem[]>("search_catalogue", {
      query,
      source: source ?? null,
    }),
  upsertItem: (input: {
    id?: number;
    rawgId: number;
    name: string;
    payload: string;
    source?: string;
  }) =>
    invoke<CatalogueItem>("upsert_catalogue_item", {
      input: {
        id: input.id,
        rawg_id: input.rawgId,
        name: input.name,
        payload: input.payload,
        source: input.source,
      },
    }),
  deleteItem: (id: number) =>
    invoke<boolean>("delete_catalogue_item", { id }),
  syncLibrary: () =>
    invoke<CatalogueSyncResult>("sync_library_to_catalogue"),
};

export const windowApi = {
  getPlatform: () => invoke<string>("get_window_platform"),
  minimize: () => invoke<void>("window_minimize"),
  toggleMaximize: () => invoke<void>("window_toggle_maximize"),
  close: () => invoke<void>("window_close"),
};
