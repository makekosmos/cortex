// Arrancador extension — typed facade над `window.kepler.arrancador.*`.
//
// Все write paths (scan, launch, RAWG apply, SQOBA backup/restore, config)
// идут через shared preload bridge. Backend регистрирует операции под
// namespace `arrancador.*` (`platform/runtime/src/arrancador/`).
//
// Если backend ещё не реализован — каждый вызов вернёт «Unknown operation»
// от ws_server.rs. UI компоненты должны это поймать (try/catch) и показать
// пользователю человеческую ошибку, а не crash'нуть.

export interface ScanResult {
  added: number;
  updated: number;
  skipped: number;
  errors: string[];
}

export interface LaunchResultOk {
  ok: true;
  pid: number;
  started_at: string;
  method: string;
}

export interface LaunchResultErr {
  ok: false;
  error: string;
}

export type LaunchResult = LaunchResultOk | LaunchResultErr;

export interface RawgGame {
  id: number;
  name: string;
  slug?: string;
  released?: string | null;
  background_image?: string | null;
  genres?: { id: number; name: string }[];
  platforms?: { platform: { id: number; name: string } }[];
}

export interface RawgSearchResult {
  results: RawgGame[];
}

export interface SqobaBackup {
  id: string;
  game_id: string;
  timestamp: string;
  dest_path: string;
  files_count: number;
  bytes: number;
}

export interface SqobaListResult {
  backups: SqobaBackup[];
}

export interface SqobaRestoreResult {
  ok: boolean;
  restored_files?: number;
  bytes?: number;
  errors?: string[];
}

export interface OkResult {
  ok: boolean;
  error?: string;
}

interface ArrancadorApi {
  scan: () => Promise<ScanResult>;
  launch: (gameId: string) => Promise<LaunchResult>;
  rawg: {
    search: (query: string) => Promise<RawgSearchResult>;
    apply: (gameId: string, rawgId: number) => Promise<OkResult>;
  };
  sqoba: {
    backup: (gameId: string) => Promise<SqobaBackup>;
    list: (gameId: string) => Promise<SqobaListResult>;
    restore: (backupId: string) => Promise<SqobaRestoreResult>;
  };
  config: {
    getRawgKey: () => Promise<{ key: string | null }>;
    setRawgKey: (key: string) => Promise<OkResult>;
  };
}

export function arrancadorApi(): ArrancadorApi | null {
  const kepler = (window as unknown as { kepler?: { arrancador?: ArrancadorApi } }).kepler;
  return kepler?.arrancador ?? null;
}

export function requireArrancadorApi(): ArrancadorApi {
  const api = arrancadorApi();
  if (!api) {
    throw new Error(
      "kepler.arrancador bridge недоступен — обновите Kepler shell (минимально API v1.x).",
    );
  }
  return api;
}
