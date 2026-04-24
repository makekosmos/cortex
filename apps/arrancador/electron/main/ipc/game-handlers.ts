import { ipcMain } from "electron";
import type { DbValue } from "../helpers/shared";
import { migrateArkGames } from "../services/ark-game-migration";
import type { WithRuntime } from "./types";

type GamePathInput = {
  name: string;
  exe_path: string;
  exe_name: string;
};

type ProcessBindingInput = {
  match_type: "exe_path" | "process_name";
  match_value: string;
};

export interface GameIpcDeps {
  withRuntime: WithRuntime;
  getArkDbPath: () => string;
}

export function registerGameIpcHandlers(deps: GameIpcDeps) {
  const { withRuntime } = deps;

  ipcMain.handle(
    "get_all_games",
    withRuntime(async ({ services }) => await services.games.getAllGames()),
  );

  ipcMain.handle(
    "get_game",
    withRuntime(
      async ({ services }, payload: { id: string }) =>
        await services.games.getGame(payload.id),
    ),
  );

  ipcMain.handle(
    "add_game",
    withRuntime(
      async ({ services }, payload: { game: GamePathInput }) =>
        await services.games.addGame(payload.game),
    ),
  );

  ipcMain.handle(
    "add_games_batch",
    withRuntime(
      async ({ services }, payload: { games: GamePathInput[] }) =>
        await services.games.addGamesBatch(payload.games),
    ),
  );

  ipcMain.handle(
    "update_game",
    withRuntime(
      async (
        { services },
        payload: { update: Record<string, DbValue> & { id: string } },
      ) => await services.games.updateGame(payload.update),
    ),
  );

  ipcMain.handle(
    "delete_game",
    withRuntime(async ({ services }, payload: { id: string }) => {
      await services.games.deleteGame(payload.id);
    }),
  );

  ipcMain.handle(
    "toggle_favorite",
    withRuntime(
      async ({ services }, payload: { id: string }) =>
        await services.games.toggleFavorite(payload.id),
    ),
  );

  ipcMain.handle(
    "get_favorites",
    withRuntime(async ({ services }) => await services.games.getFavorites()),
  );

  ipcMain.handle(
    "record_game_launch",
    withRuntime(
      async ({ services }, payload: { id: string }) =>
        await services.games.recordGameLaunch(payload.id),
    ),
  );

  ipcMain.handle(
    "search_games",
    withRuntime(
      async ({ services }, payload: { query: string }) =>
        await services.games.searchGames(payload.query),
    ),
  );

  ipcMain.handle(
    "game_exists_by_path",
    withRuntime(
      async ({ services }, payload: { exePath: string }) =>
        await services.games.gameExistsByPath(payload.exePath),
    ),
  );

  ipcMain.handle(
    "is_game_installed",
    withRuntime(
      async ({ services }, payload: { id: string }) =>
        await services.games.isGameInstalled(payload.id),
    ),
  );

  ipcMain.handle(
    "launch_game",
    withRuntime(async ({ services }, payload: { id: string }) => {
      await services.games.launchGame(payload.id);
      await services.achievements.recordAchievementEvent("game_launch");
    }),
  );

  ipcMain.handle(
    "get_running_instances",
    withRuntime(
      async ({ services }, payload: { id: string }) =>
        await services.games.getRunningInstances(payload.id),
    ),
  );

  ipcMain.handle(
    "kill_game_processes",
    withRuntime(
      async ({ services }, payload: { id: string }) =>
        await services.games.killGameProcesses(payload.id),
    ),
  );

  ipcMain.handle(
    "add_game_process_bindings",
    withRuntime(
      async (
        { services },
        payload: { id: string; bindings: ProcessBindingInput[] },
      ) => await services.games.addProcessBindings(payload.id, payload.bindings),
    ),
  );

  ipcMain.handle(
    "remove_game_process_binding",
    withRuntime(
      async ({ services }, payload: { id: string; bindingId: number }) =>
        await services.games.removeProcessBinding(payload.id, payload.bindingId),
    ),
  );

  ipcMain.handle(
    "resolve_shortcut_target",
    withRuntime(
      async ({ services }, payload: { path: string }) =>
        await services.games.resolveShortcutTarget(payload.path),
    ),
  );

  ipcMain.handle(
    "list_recent_usage_processes",
    withRuntime(
      async ({ services }, payload: { limit?: number } = {}) =>
        await services.usageProcessSearch.getRecentProcesses(payload?.limit),
    ),
  );

  ipcMain.handle(
    "search_usage_processes",
    withRuntime(
      async ({ services }, payload: { query: string; limit?: number }) =>
        await services.usageProcessSearch.searchProcesses(
          payload.query,
          payload.limit,
        ),
    ),
  );

  ipcMain.handle(
    "sync_games_to_ark",
    withRuntime(async ({ services }) => await services.games.syncAllGamesToArk()),
  );

  ipcMain.handle(
    "migrate_ark_games",
    withRuntime(async ({ services }, payload: { sourceDbPath: string }) => {
      const result = await migrateArkGames({
        sourceDbPath: payload.sourceDbPath,
        targetDbPath: deps.getArkDbPath(),
      });
      await services.games.syncAllGamesToArk();
      return result;
    }),
  );

  ipcMain.handle(
    "search_rawg",
    withRuntime(
      async ({ services }, payload: { query: string }) =>
        await services.metadata.searchRawg(payload.query),
    ),
  );

  ipcMain.handle(
    "get_rawg_game_details",
    withRuntime(
      async ({ services }, payload: { rawgId: number }) =>
        await services.metadata.getRawgGameDetails(payload.rawgId),
    ),
  );

  ipcMain.handle(
    "apply_rawg_metadata",
    withRuntime(
      async (
        { services },
        payload: { gameId: string; rawgId: number; rename: boolean },
      ) =>
        await services.metadata.applyRawgMetadata(
          payload.gameId,
          payload.rawgId,
          payload.rename,
        ),
    ),
  );

  ipcMain.handle(
    "set_rawg_api_key",
    withRuntime(async ({ services }, payload: { key: string }) => {
      await services.metadata.setRawgApiKey(payload.key);
    }),
  );

  ipcMain.handle(
    "get_rawg_api_key",
    withRuntime(async ({ services }) => await services.metadata.getRawgApiKey()),
  );

  ipcMain.handle(
    "get_all_achievements",
    withRuntime(
      async ({ services }, payload: { unlockedOnly: boolean }) =>
        await services.achievements.getAllAchievements(payload.unlockedOnly),
    ),
  );

  ipcMain.handle(
    "seed_default_achievements",
    withRuntime(
      async ({ services }) => await services.achievements.seedDefaultAchievements(),
    ),
  );

  ipcMain.handle(
    "record_achievement_event",
    withRuntime(
      async (
        { services },
        payload: { eventTrigger: string; eventContext?: string },
      ) =>
        await services.achievements.recordAchievementEvent(
          payload.eventTrigger,
          payload.eventContext,
        ),
    ),
  );
}
