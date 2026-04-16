export interface Game {
  id: string;
  name: string;
  exe_path: string;
  exe_name: string;
  play_status: "not_started" | "in_progress" | "completed" | "abandoned";

  // RAWG metadata
  rawg_id: number | null;
  description: string | null;
  released: string | null;
  background_image: string | null;
  metacritic: number | null;
  rating: number | null;
  genres: string | null;
  platforms: string | null;
  developers: string | null;
  publishers: string | null;

  // Local metadata
  cover_image: string | null;
  icon_image: string | null;
  is_favorite: boolean;
  play_count: number;
  total_playtime: number;
  last_played: string | null;
  date_added: string;

  // Backup
  backup_enabled: boolean;
  last_backup: string | null;
  backup_count: number;
  save_path: string | null;

  user_rating: number | null;
  user_note: string | null;
}

export interface Achievement {
  id: string;
  title: string;
  description: string;
  event_trigger: string;
  progress: number;
  target: number;
  unlocked: boolean;
  unlocked_at: string | null;
  created_at: string;
}

export const testGameFixture: Game = {
  id: "game-1",
  name: "Arcadia",
  exe_path: "C:\\Games\\Arcadia\\arcadia.exe",
  exe_name: "arcadia.exe",
  rawg_id: 1101,
  description: "Test game description",
  released: "2022-01-12",
  background_image: null,
  metacritic: 86,
  rating: 4.3,
  genres: "Action, RPG",
  platforms: "PC",
  developers: "Arcadia Studio",
  publishers: "Arcadia Publishing",
  cover_image: null,
  icon_image: null,
  is_favorite: false,
  play_count: 2,
  total_playtime: 5400,
  play_status: "in_progress",
  last_played: "2024-01-03T12:00:00.000Z",
  date_added: "2024-01-01T09:00:00.000Z",
  backup_enabled: true,
  last_backup: "2024-01-02T09:00:00.000Z",
  backup_count: 1,
  save_path: "C:\\Games\\Arcadia\\saves",
  user_rating: 5,
  user_note: "Great game",
};

export const testFavoriteGameFixture: Game = {
  ...testGameFixture,
  id: "game-2",
  name: "Bastion",
  exe_path: "C:\\Games\\Bastion\\bastion.exe",
  exe_name: "bastion.exe",
  rawg_id: 1102,
  is_favorite: true,
  play_count: 0,
  total_playtime: 0,
  play_status: "not_started",
  last_played: null,
  metacritic: 74,
};

export const createTestGame = (overrides: Partial<Game> = {}): Game => ({
  ...testGameFixture,
  ...overrides,
});

export interface NewGame {
  name: string;
  exe_path: string;
  exe_name: string;
}

export interface UpdateGame {
  id: string;
  name?: string | null;
  exe_path?: string;
  description?: string | null;
  cover_image?: string | null;
  icon_image?: string | null;
  is_favorite?: boolean;
  backup_enabled?: boolean;
  save_path?: string | null;
  rawg_id?: number | null;
  released?: string | null;
  background_image?: string | null;
  metacritic?: number | null;
  rating?: number | null;
  genres?: string | null;
  platforms?: string | null;
  developers?: string | null;
  publishers?: string | null;
  user_rating?: number | null;
  user_note?: string | null;
  play_status?: "not_started" | "in_progress" | "completed" | "abandoned";
}

export interface Backup {
  id: string;
  game_id: string;
  backup_path: string;
  backup_size: number;
  created_at: string;
  is_auto: boolean;
  notes: string | null;
}

export interface BackupInfo {
  game_name: string;
  save_path: string | null;
  registry_path: string | null;
  total_size: number;
  files: string[];
}

export interface SavePathLookup {
  save_path: string | null;
  candidates: string[];
}

export interface RestoreCheck {
  should_restore: boolean;
  backup_id: string | null;
  current_size: number;
  backup_size: number;
}

export interface RawgGame {
  id: number;
  name: string;
  slug: string;
  released: string | null;
  background_image: string | null;
  metacritic: number | null;
  rating: number | null;
  ratings_count: number | null;
  genres: { id: number; name: string; slug: string }[] | null;
  platforms:
    | {
        platform: { id: number; name: string; slug: string };
        requirements_en?: string | null;
        requirements_ru?: string | null;
      }[]
    | null;
}

export interface RawgGameDetails extends RawgGame {
  description: string | null;
  description_raw: string | null;
  background_image_additional: string | null;
  developers: { id: number; name: string; slug: string }[] | null;
  publishers: { id: number; name: string; slug: string }[] | null;
  stores:
    | {
        id: number;
        url: string | null;
        store?: {
          id: number;
          name: string;
          slug: string;
          domain?: string | null;
        } | null;
      }[]
    | null;
}

export interface AppSettings {
  theme: string;
  ludusavi_path: string;
  backup_directory: string;
  auto_backup: boolean;
  backup_before_launch: boolean;
  backup_compression_enabled: boolean;
  backup_compression_level: number;
  backup_skip_compression_once: boolean;
  max_backups_per_game: number;
  rawg_api_key: string;
  start_minimized_in_tray: boolean;
}

export interface ExeEntry {
  path: string;
  file_name: string;
}

export interface ProcessEntry {
  pid: number;
  name: string;
  path: string;
  cpu_usage: number;
  gpu_usage: number;
}

export interface DailyPlaytime {
  date: string;
  seconds: number;
}

export interface GamePlaytime {
  id: string;
  name: string;
  seconds: number;
}

export interface PlaytimeStats {
  range_start: string;
  range_end: string;
  total_seconds: number;
  daily_totals: DailyPlaytime[];
  per_game_totals: GamePlaytime[];
}

export interface SystemCpuInfo {
  brand: string;
  vendor_id: string;
  frequency_mhz: number;
  physical_cores: number | null;
  logical_cores: number;
}

export interface SystemMemoryInfo {
  total_bytes: number;
  used_bytes: number;
  free_bytes: number;
  available_bytes: number;
  total_swap_bytes: number;
  used_swap_bytes: number;
}

export interface SystemDiskInfo {
  name: string;
  mount_point: string;
  file_system: string;
  total_bytes: number;
  available_bytes: number;
  kind: string;
  is_removable: boolean;
  model: string | null;
  media_type: string | null;
}

export interface SystemGpuInfo {
  name: string;
  device_name: string;
  is_primary: boolean;
}

export interface SystemMonitorInfo {
  name: string;
  device_name: string;
  width: number;
  height: number;
  refresh_rate: number;
  is_primary: boolean;
}

export interface SystemInfo {
  hostname: string | null;
  os_name: string | null;
  os_version: string | null;
  kernel_version: string | null;
  uptime_seconds: number;
  boot_time: number;
  arch: string;
  cpu: SystemCpuInfo;
  memory: SystemMemoryInfo;
  disks: SystemDiskInfo[];
  gpus: SystemGpuInfo[];
  monitors: SystemMonitorInfo[];
}

export interface DiskSpeedResult {
  mount_point: string;
  size_bytes: number;
  write_mbps: number;
  read_mbps: number;
  elapsed_write_ms: number;
  elapsed_read_ms: number;
}

export interface NotificationItem {
  id: string;
  level: string;
  title: string;
  message: string;
  source: string | null;
  created_at: string;
  read_at: string | null;
}

export interface CatalogueItem {
  id: number;
  rawg_id: number;
  name: string;
  payload: string;
  source: string;
  updated_at: string;
}

export interface CatalogueSyncResult {
  synced: number;
}
