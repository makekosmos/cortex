export type GameProcessBindingMatchType = "exe_path" | "process_name";

export interface GameProcessBinding {
  id: number;
  game_id: string;
  match_type: GameProcessBindingMatchType;
  match_value: string;
  created_at: string;
}

export interface Game {
  id: string;
  ark_object_id: string | null;
  name: string;
  exe_path: string;
  exe_name: string;
  process_bindings: GameProcessBinding[];
  play_status: string;

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
}

export interface NewGame {
  name: string;
  exe_path: string;
  exe_name: string;
}

export interface UpdateGame {
  id: string;
  ark_object_id?: string | null;
  name?: string | null;
  exe_path?: string | null;
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
  play_status?: string | null;
  user_rating?: number | null;
  user_note?: string | null;
}

export interface NewGameProcessBinding {
  match_type: GameProcessBindingMatchType;
  match_value: string;
}

export interface RunningProcessInfo {
  pid: number;
  name: string;
  path: string;
}
