import type { DbLike } from "../helpers/shared";
import type { PlaytimeStatsRepository } from "./contracts";
import { createPlaytimeStatsRepository as createArkBackedPlaytimeStatsRepository } from "./ark-usage";

// Keep the read-model behind a single factory so the backing source can change
// without changing IPC or page-level contracts.
export function createPlaytimeStatsRepository(
  db: DbLike,
  arkDbPath: string,
): PlaytimeStatsRepository {
  return createArkBackedPlaytimeStatsRepository({
    legacyDb: db,
    arkDbPath,
  });
}
