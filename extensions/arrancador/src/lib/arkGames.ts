// Arrancador extension — shared ARK projection helpers.
//
// Чтение game_obj из ARK через `window.kepler.ark`. Не пишет обратно
// (write paths остаются в legacy `apps/arrancador/` до Phase 5).
//
// Адаптация vs `apps/arrancador/src-vue/stores/games.ts`:
//   - legacy: Pinia store + electronAPI.gamesApi.* (full CRUD).
//   - extension: read-only projection из `list_objects_by_type`.

import type { ArkObjectRecord } from "@kosmos/ark";

export interface ArrancadorGame {
  id: string;
  name: string;
  coverImage: string | null;
  backgroundImage: string | null;
  description: string | null;
  genres: string | null;
  platforms: string | null;
  released: string | null;
  isFavorite: boolean;
  totalPlaytime: number | null;
  userRating: number | null;
  userNote: string | null;
  playStatus: string | null;
  exePath: string | null;
  rawgId: number | null;
}

export interface KeplerArkBridge {
  request: (operation: string, params?: Record<string, unknown>) => Promise<unknown>;
  subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
}

export function arkBridge(): KeplerArkBridge | null {
  const kepler = (window as unknown as { kepler?: { ark?: KeplerArkBridge } }).kepler;
  return kepler?.ark ?? null;
}

function readString(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function readNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function readBoolean(value: unknown): boolean {
  return value === true;
}

export function projectGame(record: ArkObjectRecord): ArrancadorGame {
  const props =
    record.propsJson && typeof record.propsJson === "object"
      ? (record.propsJson as Record<string, unknown>)
      : {};
  return {
    id: record.id,
    name: record.title ?? readString(props.name) ?? "Без названия",
    coverImage: readString(props.cover_image),
    backgroundImage: readString(props.background_image),
    description: readString(props.description),
    genres: readString(props.genres),
    platforms: readString(props.platforms),
    released: readString(props.released),
    isFavorite: readBoolean(props.is_favorite),
    totalPlaytime: readNumber(props.total_playtime),
    userRating: readNumber(props.user_rating),
    userNote: readString(props.user_note),
    playStatus: readString(props.play_status),
    exePath: readString(props.exe_path),
    rawgId: readNumber(props.rawg_id),
  };
}

export async function loadGames(): Promise<ArrancadorGame[]> {
  const bridge = arkBridge();
  if (!bridge) {
    throw new Error("kepler.ark bridge недоступен");
  }
  const result = await bridge.request("list_objects_by_type", {
    type_id: "game_obj",
  });
  const records = Array.isArray(result) ? (result as ArkObjectRecord[]) : [];
  return records
    .filter((r) => r.deletedAt === null)
    .map(projectGame)
    .sort((a, b) => a.name.localeCompare(b.name, "ru"));
}
