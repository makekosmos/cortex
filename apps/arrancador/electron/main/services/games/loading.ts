import { queryOne } from "../../helpers/db";
import type { DbLike } from "../../helpers/shared";
import { listGameProcessBindingsByGameId } from "../game-process-bindings";
import { GAME_SELECT, type GameDbRow, mapGameRow } from "./rows";
import type { Game } from "./types";

async function getRowById(db: DbLike, id: string): Promise<Game | null> {
  const row = await queryOne<GameDbRow>(db, `${GAME_SELECT} WHERE id = ?1`, [id]);
  return row ? mapGameRow(row) : null;
}

async function hydrateExplicitBindings(
  db: DbLike,
  games: Game[],
): Promise<Game[]> {
  if (games.length === 0) {
    return games;
  }

  const bindingsByGameId = await listGameProcessBindingsByGameId(
    db,
    games.map((game) => game.id),
  );

  return games.map((game) => ({
    ...game,
    process_bindings: bindingsByGameId.get(game.id) ?? [],
  }));
}

export function createGameLoaders(db: DbLike) {
  return {
    async loadGame(id: string) {
      const fetched = await getRowById(db, id);
      if (!fetched) {
        return null;
      }
      const [withBindings] = await hydrateExplicitBindings(db, [fetched]);
      return withBindings ?? null;
    },

    async loadGames(rows: GameDbRow[]) {
      return await hydrateExplicitBindings(db, rows.map(mapGameRow));
    },
  };
}
