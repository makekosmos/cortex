import { shallowRef, watch } from "vue";
import { gamesApi } from "../../src/lib/api";
import type { Game } from "../../src/types";

export type LibraryInstallGame = Pick<Game, "id" | "exe_path">;

type InstallStatusCacheEntry = {
  signature: string;
  installed: boolean;
};

export interface LibraryInstallStatusOptions {
  games: () => readonly LibraryInstallGame[];
  checkInstalled?: (id: string) => Promise<boolean>;
  concurrency?: number;
  fallbackInstalled?: boolean;
  log?: Pick<Console, "error">;
}

const DEFAULT_CONCURRENCY = 6;

export function createInstallStatusSignature(game: LibraryInstallGame): string {
  return `${game.id}\u0000${game.exe_path}`;
}

export async function mapWithConcurrency<T, R>(
  items: readonly T[],
  concurrency: number,
  worker: (item: T) => Promise<R>,
): Promise<R[]> {
  if (items.length === 0) {
    return [];
  }

  const limit = Math.max(1, Math.floor(concurrency));
  const results = new Array<R>(items.length);
  let nextIndex = 0;

  async function runWorker() {
    while (nextIndex < items.length) {
      const index = nextIndex;
      nextIndex += 1;
      const item = items[index];
      if (item !== undefined) {
        results[index] = await worker(item);
      }
    }
  }

  await Promise.all(
    Array.from({ length: Math.min(limit, items.length) }, () => runWorker()),
  );

  return results;
}

export function useLibraryInstallStatus(options: LibraryInstallStatusOptions) {
  const installedById = shallowRef<Record<string, boolean>>({});
  const cache = new Map<string, InstallStatusCacheEntry>();
  const checkInstalled = options.checkInstalled ?? gamesApi.isInstalled;
  const concurrency = options.concurrency ?? DEFAULT_CONCURRENCY;
  const fallbackInstalled = options.fallbackInstalled ?? true;
  const log = options.log ?? console;
  let requestId = 0;

  async function refreshInstallStatuses() {
    const currentRequestId = ++requestId;
    const games = options.games();

    if (games.length === 0) {
      cache.clear();
      installedById.value = {};
      return;
    }

    const liveIds = new Set(games.map((game) => game.id));
    for (const id of cache.keys()) {
      if (!liveIds.has(id)) {
        cache.delete(id);
      }
    }

    const nextInstalledById: Record<string, boolean> = {};
    const gamesToCheck: LibraryInstallGame[] = [];

    for (const game of games) {
      const signature = createInstallStatusSignature(game);
      const cached = cache.get(game.id);
      if (cached?.signature === signature) {
        nextInstalledById[game.id] = cached.installed;
        continue;
      }

      gamesToCheck.push(game);
    }

    if (gamesToCheck.length === 0) {
      installedById.value = nextInstalledById;
      return;
    }

    const checked = await mapWithConcurrency(
      gamesToCheck,
      concurrency,
      async (game) => {
        try {
          return {
            id: game.id,
            signature: createInstallStatusSignature(game),
            installed: await checkInstalled(game.id),
          };
        } catch (cause) {
          log.error?.("Failed to check install status:", cause);
          return {
            id: game.id,
            signature: createInstallStatusSignature(game),
            installed: fallbackInstalled,
          };
        }
      },
    );

    if (currentRequestId !== requestId) {
      return;
    }

    for (const result of checked) {
      cache.set(result.id, {
        signature: result.signature,
        installed: result.installed,
      });
      nextInstalledById[result.id] = result.installed;
    }

    installedById.value = nextInstalledById;
  }

  watch(
    () => options.games().map(createInstallStatusSignature).join("\n"),
    () => {
      void refreshInstallStatuses();
    },
    { immediate: true },
  );

  return {
    installedById,
    refreshInstallStatuses,
  };
}
