export type MaybePromise<T> = T | Promise<T>;

export type DbValue = string | number | null | Uint8Array | ArrayBuffer;

export interface DbRunResult {
  changes: number;
  lastInsertRowid?: number | bigint;
}

export interface DbLike {
  all<T = Record<string, unknown>>(
    sql: string,
    params?: readonly DbValue[],
  ): MaybePromise<T[]>;
  get<T = Record<string, unknown> | undefined>(
    sql: string,
    params?: readonly DbValue[],
  ): MaybePromise<T | undefined>;
  run(sql: string, params?: readonly DbValue[]): MaybePromise<DbRunResult>;
  transaction?<T>(fn: (tx: DbLike) => MaybePromise<T>): MaybePromise<T>;
}

export type GamePlayStatus =
  | "not_started"
  | "in_progress"
  | "completed"
  | "abandoned";

export interface GameSnapshot {
  id: string;
  name: string;
  exe_path: string;
  exe_name: string;
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
  cover_image: string | null;
  icon_image: string | null;
  is_favorite: boolean;
  play_count: number;
  total_playtime: number;
  last_played: string | null;
  date_added: string;
  backup_enabled: boolean;
  last_backup: string | null;
  backup_count: number;
  save_path: string | null;
  user_rating: number | null;
  user_note: string | null;
  play_status: GamePlayStatus;
}

export interface RawgGenre {
  id: number;
  name: string;
  slug: string;
}

export interface RawgPlatform {
  id: number;
  name: string;
  slug: string;
}

export interface RawgPlatformWrapper {
  platform: RawgPlatform;
  requirements_en?: string | null;
  requirements_ru?: string | null;
}

export interface RawgDeveloper {
  id: number;
  name: string;
  slug: string;
}

export interface RawgPublisher {
  id: number;
  name: string;
  slug: string;
}

export interface RawgStore {
  id: number;
  name: string;
  slug: string;
  domain?: string | null;
}

export interface RawgStoreWrapper {
  id: number;
  url?: string | null;
  store?: RawgStore | null;
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
  genres: RawgGenre[] | null;
  platforms: RawgPlatformWrapper[] | null;
}

export interface RawgGameDetails extends RawgGame {
  description: string | null;
  description_raw: string | null;
  background_image_additional: string | null;
  developers: RawgDeveloper[] | null;
  publishers: RawgPublisher[] | null;
  stores: RawgStoreWrapper[] | null;
}

export interface RawgMetadataUpdate {
  gameId: string;
  rawgId: number;
  name: string | null;
  description: string | null;
  released: string | null;
  background_image: string | null;
  metacritic: number | null;
  rating: number | null;
  genres: string | null;
  platforms: string | null;
  developers: string | null;
  publishers: string | null;
}

export interface RawgApiKeyStore {
  getRawgApiKey(): MaybePromise<string | null | undefined>;
  setRawgApiKey(key: string): MaybePromise<void>;
}

export interface MetadataGamePort {
  applyRawgMetadata(update: RawgMetadataUpdate): MaybePromise<GameSnapshot>;
}

export interface CatalogueItem {
  id: number;
  rawg_id: number;
  name: string;
  payload: string;
  source: string;
  updated_at: string;
}

export interface CatalogueLibraryItem {
  id: string;
  name: string;
  exe_name: string;
}

export interface CatalogueLibraryPort {
  listGames(): MaybePromise<CatalogueLibraryItem[]>;
}

export interface CatalogueSyncResult {
  synced: number;
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

export interface AchievementSeed {
  id: string;
  title: string;
  description: string;
  event_trigger: string;
  target: number;
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
