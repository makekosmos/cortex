import type { DbLike } from "../../helpers/shared";
import type { ProcessMatch } from "./process";
import type {
  Game,
  NewGame,
  NewGameProcessBinding,
  UpdateGame,
} from "./types";

export interface GamesServiceDeps {
  db: DbLike;
  usageReadModel?: {
    hydrateGame(game: Game): Promise<Game>;
    hydrateGames(games: Game[]): Promise<Game[]>;
  };
  arkGameObjectSync?: {
    syncGame(game: Game): Promise<string | null>;
  };
  now?: () => Date;
  fileExists?: (filePath: string) => boolean;
  log?: Pick<Console, "error" | "warn">;
  resolveShortcutTarget?: (inputPath: string) => Promise<string>;
  countRunningInstances?: (matches: ProcessMatch[]) => Promise<number>;
  killMatchingProcesses?: (matches: ProcessMatch[]) => Promise<number>;
  spawnGameProcess?: (exePath: string) => Promise<void>;
}

export interface GamesService {
  getGame(id: string): Promise<Game | null>;
  addGame(game: NewGame): Promise<Game>;
  addGamesBatch(games: NewGame[]): Promise<Game[]>;
  getAllGames(): Promise<Game[]>;
  getFavorites(): Promise<Game[]>;
  updateGame(update: UpdateGame): Promise<Game>;
  toggleFavorite(id: string): Promise<Game>;
  deleteGame(id: string): Promise<void>;
  /**
   * Legacy renderer contract. Usage capture is owned by Ark/usage-tracker;
   * this returns the current Ark-hydrated game snapshot after a launch event.
   */
  recordGameLaunch(id: string): Promise<Game>;
  searchGames(query: string): Promise<Game[]>;
  gameExistsByPath(exePath: string): Promise<boolean>;
  resolveShortcutTarget(path: string): Promise<string>;
  isGameInstalled(id: string): Promise<boolean>;
  getRunningInstances(id: string): Promise<number>;
  killGameProcesses(id: string): Promise<number>;
  launchGame(id: string): Promise<void>;
  addProcessBindings(
    id: string,
    bindings: NewGameProcessBinding[],
  ): Promise<Game>;
  removeProcessBinding(id: string, bindingId: number): Promise<Game>;
  syncAllGamesToArk(): Promise<GamesArkSyncResult>;
}

export interface GamesArkSyncResult {
  total: number;
  synced: number;
  failed: number;
}
