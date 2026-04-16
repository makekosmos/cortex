import {
  buildRawgMetadataSummary,
  createRawgClient,
  type RawgClient,
} from "../helpers/rawg";
import type {
  GameSnapshot,
  MetadataGamePort,
  RawgApiKeyStore,
  RawgGame,
  RawgGameDetails,
  RawgMetadataUpdate,
} from "../helpers/shared";

export interface MetadataServiceDeps {
  settings: RawgApiKeyStore;
  games: MetadataGamePort;
  rawg?: RawgClient;
  fetchImpl?: typeof fetch;
  timeoutMs?: number;
  userAgent?: string;
}

export interface MetadataService {
  searchRawg(query: string): Promise<RawgGame[]>;
  getRawgGameDetails(rawgId: number): Promise<RawgGameDetails>;
  applyRawgMetadata(gameId: string, rawgId: number, rename: boolean): Promise<GameSnapshot>;
  setRawgApiKey(key: string): Promise<void>;
  getRawgApiKey(): Promise<string>;
}

function createMetadataUpdate(
  gameId: string,
  rawgId: number,
  rename: boolean,
  details: RawgGameDetails,
): RawgMetadataUpdate {
  const summary = buildRawgMetadataSummary(details);

  return {
    gameId,
    rawgId,
    name: rename ? summary.name : null,
    description: summary.description,
    released: summary.released,
    background_image: summary.background_image,
    metacritic: summary.metacritic,
    rating: summary.rating,
    genres: summary.genres,
    platforms: summary.platforms,
    developers: summary.developers,
    publishers: summary.publishers,
  };
}

export function createMetadataService(deps: MetadataServiceDeps): MetadataService {
  const rawg =
    deps.rawg ??
    createRawgClient({
      getApiKey: () => deps.settings.getRawgApiKey(),
      fetchImpl: deps.fetchImpl,
      timeoutMs: deps.timeoutMs,
      userAgent: deps.userAgent,
    });

  return {
    async searchRawg(query: string): Promise<RawgGame[]> {
      return await rawg.searchRawg(query);
    },

    async getRawgGameDetails(rawgId: number): Promise<RawgGameDetails> {
      return await rawg.getRawgGameDetails(rawgId);
    },

    async applyRawgMetadata(
      gameId: string,
      rawgId: number,
      rename: boolean,
    ): Promise<GameSnapshot> {
      const details = await rawg.getRawgGameDetails(rawgId);
      const update = createMetadataUpdate(gameId, rawgId, rename, details);
      return await deps.games.applyRawgMetadata(update);
    },

    async setRawgApiKey(key: string): Promise<void> {
      await deps.settings.setRawgApiKey(key);
    },

    async getRawgApiKey(): Promise<string> {
      return (await deps.settings.getRawgApiKey()) ?? "";
    },
  };
}
