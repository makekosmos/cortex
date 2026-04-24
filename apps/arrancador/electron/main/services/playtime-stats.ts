import type { DbLike } from "../helpers/shared";
import { createPlaytimeStatsRepository as createArkBackedPlaytimeStatsRepository } from "./ark-usage";
import type { PlaytimeStatsRepository } from "./contracts";

// Keep the read-model behind a single factory so the backing source can change
// without changing IPC or page-level contracts.
export function createPlaytimeStatsRepository(
  db: DbLike,
  arkDbPath: string,
  fallbackArkDbPath?: string,
): PlaytimeStatsRepository {
  return createArkBackedPlaytimeStatsRepository({
    legacyDb: db,
    arkDbPath,
    fallbackArkDbPath,
  });
}
