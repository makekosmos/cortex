import type {
  Achievement,
  AppSettings,
  Backup,
  BackupInfo,
  CatalogueItem,
  CatalogueSyncResult,
  DiskSpeedResult,
  Game,
  NewGame,
  NotificationItem,
  PlaytimeStats,
  ProcessEntry,
  RawgGame,
  RawgGameDetails,
  RestoreCheck,
  SavePathLookup,
  SystemInfo,
  UpdateGame,
} from "./index";

export interface IpcRequestMap {
  get_all_games: undefined;
  get_game: { id: string };
  add_game: { game: NewGame };
  add_games_batch: { games: NewGame[] };
  update_game: { update: UpdateGame };
  delete_game: { id: string };
  toggle_favorite: { id: string };
  get_favorites: undefined;
  record_game_launch: { id: string };
  search_games: { query: string };
  game_exists_by_path: { exePath: string };
  is_game_installed: { id: string };
  launch_game: { id: string };
  get_running_instances: { id: string };
  kill_game_processes: { id: string };
  resolve_shortcut_target: { path: string };

  search_rawg: { query: string };
  get_rawg_game_details: { rawgId: number };
  apply_rawg_metadata: { gameId: string; rawgId: number; rename: boolean };
  set_rawg_api_key: { key: string };
  get_rawg_api_key: undefined;

  get_all_achievements: { unlockedOnly: boolean };
  seed_default_achievements: undefined;
  record_achievement_event: {
    eventTrigger: string;
    eventContext?: string;
  };

  check_ludusavi_installed: undefined;
  get_ludusavi_executable_path: undefined;
  set_ludusavi_path: { path: string };
  set_backup_directory: { path: string };
  get_backup_directory_setting: undefined;
  refresh_sqoba_manifest: undefined;
  find_game_save_paths: { gameName: string; gameId?: string };
  find_game_saves: { gameName: string; gameId?: string };
  create_backup: {
    gameId: string;
    gameName: string;
    isAuto: boolean;
    notes?: string;
  };
  get_game_backups: { gameId: string };
  restore_backup: { backupId: string };
  delete_backup: { backupId: string };
  should_backup_before_launch: { gameId: string };
  check_backup_needed: { gameId: string; gameName: string };
  check_restore_needed: { gameId: string; gameName: string };
  get_backup_settings: undefined;
  update_backup_settings: { settings: Record<string, string> };

  get_all_settings: undefined;
  update_settings: { settings: AppSettings };
  get_setting: { key: string };
  set_setting: { key: string; value: string };
  add_scan_directory: { path: string };
  get_scan_directories: undefined;
  remove_scan_directory: { path: string };

  get_playtime_stats: { start?: string; end?: string };
  get_running_processes: undefined;
  get_system_info: undefined;
  test_disk_speed: { mountPoint: string };

  list_notifications: { unread_only: boolean };
  create_notification: {
    level: string;
    title: string;
    message: string;
    source?: string;
  };
  mark_notification_read: { id: string };
  mark_all_notifications_read: undefined;
  clear_notifications: undefined;

  get_catalogue_items: { source: string | null };
  search_catalogue: { query: string; source: string | null };
  upsert_catalogue_item: {
    input: {
      id?: number;
      rawg_id: number;
      name: string;
      payload: string;
      source?: string;
    };
  };
  delete_catalogue_item: { id: number };
  sync_library_to_catalogue: undefined;

  dialog_open: {
    directory?: boolean;
    multiple?: boolean;
    title?: string;
    filters?: Array<{ name: string; extensions: string[] }>;
  };
  shell_open_path: { path: string };
  shell_open_external: { url: string };
  get_autostart_state: undefined;
  set_autostart_state: { enabled: boolean };
  scan_executables_stream: { dir: string };
  cancel_scan: undefined;
}

export interface IpcResultMap {
  get_all_games: Game[];
  get_game: Game | null;
  add_game: Game;
  add_games_batch: Game[];
  update_game: Game;
  delete_game: void;
  toggle_favorite: Game;
  get_favorites: Game[];
  record_game_launch: Game;
  search_games: Game[];
  game_exists_by_path: boolean;
  is_game_installed: boolean;
  launch_game: void;
  get_running_instances: number;
  kill_game_processes: number;
  resolve_shortcut_target: string;

  search_rawg: RawgGame[];
  get_rawg_game_details: RawgGameDetails;
  apply_rawg_metadata: Game;
  set_rawg_api_key: void;
  get_rawg_api_key: string;

  get_all_achievements: Achievement[];
  seed_default_achievements: Achievement[];
  record_achievement_event: Achievement[];

  check_ludusavi_installed: boolean;
  get_ludusavi_executable_path: string | null;
  set_ludusavi_path: void;
  set_backup_directory: void;
  get_backup_directory_setting: string;
  refresh_sqoba_manifest: void;
  find_game_save_paths: SavePathLookup;
  find_game_saves: BackupInfo | null;
  create_backup: Backup;
  get_game_backups: Backup[];
  restore_backup: void;
  delete_backup: void;
  should_backup_before_launch: boolean;
  check_backup_needed: boolean;
  check_restore_needed: RestoreCheck;
  get_backup_settings: Record<string, string>;
  update_backup_settings: void;

  get_all_settings: AppSettings;
  update_settings: void;
  get_setting: string | null;
  set_setting: void;
  add_scan_directory: void;
  get_scan_directories: string[];
  remove_scan_directory: void;

  get_playtime_stats: PlaytimeStats;
  get_running_processes: ProcessEntry[];
  get_system_info: SystemInfo;
  test_disk_speed: DiskSpeedResult;

  list_notifications: NotificationItem[];
  create_notification: NotificationItem;
  mark_notification_read: boolean;
  mark_all_notifications_read: number;
  clear_notifications: number;

  get_catalogue_items: CatalogueItem[];
  search_catalogue: CatalogueItem[];
  upsert_catalogue_item: CatalogueItem;
  delete_catalogue_item: boolean;
  sync_library_to_catalogue: CatalogueSyncResult;

  dialog_open: string | string[] | null;
  shell_open_path: string;
  shell_open_external: void;
  get_autostart_state: boolean;
  set_autostart_state: void;
  scan_executables_stream: number;
  cancel_scan: void;
}

export type IpcChannel = keyof IpcResultMap;
export type IpcArgs<C extends IpcChannel> = IpcRequestMap[C];
export type NoArgIpcChannel = {
  [K in IpcChannel]-?: IpcArgs<K> extends undefined ? K : never;
}[IpcChannel];
export type ArgIpcChannel = Exclude<IpcChannel, NoArgIpcChannel>;

export type IpcInvoke = {
  <C extends NoArgIpcChannel>(channel: C): Promise<IpcResultMap[C]>;
  <C extends ArgIpcChannel>(channel: C, payload: IpcArgs<C>): Promise<IpcResultMap[C]>;
};

export interface ArrancadorEventMap {
  "scan:entry": { path: string; file_name: string };
  "scan:done": { count: number };
  "backup:progress": {
    game_id: string;
    stage: string;
    message: string;
    done: number;
    total: number;
  };
  "restore:progress": {
    game_id: string;
    stage: string;
    message: string;
    done: number;
    total: number;
  };
  "game:save-path-missing": {
    game_id: string;
    game_name: string;
  };
}

export type ArrancadorEventName = keyof ArrancadorEventMap;
export type ArrancadorEventCallback<E extends ArrancadorEventName> = (
  payload: ArrancadorEventMap[E],
) => void;

export interface ArrancadorBridge {
  invoke: IpcInvoke;
  on<E extends ArrancadorEventName>(
    event: E,
    callback: ArrancadorEventCallback<E>,
  ): () => void;
}

export const ARRANCADOR_BRIDGE_KEY = "arrancador" as const;
