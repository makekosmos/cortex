export interface LookupState<T> {
  loading: boolean;
  data?: T;
  error?: string;
}

export const GAME_PATH_TOKEN = "{PATHTOGAME}";
