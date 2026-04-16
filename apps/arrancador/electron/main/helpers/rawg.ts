import type {
  RawgDeveloper,
  RawgGame,
  RawgGameDetails,
  RawgPublisher,
} from "./shared";

const RAWG_API_BASE = "https://api.rawg.io/api";
const DEFAULT_TIMEOUT_MS = 15_000;
const DEFAULT_USER_AGENT = "Arrancador/0.1.0";

export interface RawgClientOptions {
  getApiKey: () => string | null | undefined | Promise<string | null | undefined>;
  fetchImpl?: typeof fetch;
  timeoutMs?: number;
  userAgent?: string;
}

export interface RawgClient {
  searchRawg(query: string): Promise<RawgGame[]>;
  getRawgGameDetails(rawgId: number): Promise<RawgGameDetails>;
}

export interface RawgMetadataSummary {
  name: string | null;
  description: string | null;
  released: string | null;
  background_image: string | null;
  metacritic: number | null;
  rating: number | null;
  genres: string | null;
  platforms: string | null;
  developers: string | null;
  publishers: string | null;
}

type FetchLike = typeof fetch;

function joinNames<T extends { name: string }>(items: T[] | null | undefined): string | null {
  if (!items || items.length === 0) {
    return null;
  }

  return items.map((item) => item.name).join(", ");
}

function buildApiUrl(path: string, apiKey: string | null | undefined): string {
  const normalizedPath = path.startsWith("/") ? path.slice(1) : path;
  const url = new URL(normalizedPath, `${RAWG_API_BASE}/`);
  if (apiKey && apiKey.trim().length > 0) {
    url.searchParams.set("key", apiKey.trim());
  }
  return url.toString();
}

async function requestJson<T>(
  fetchImpl: FetchLike,
  apiKey: string | null | undefined,
  path: string,
  timeoutMs: number,
  userAgent: string,
): Promise<T> {
  const url = buildApiUrl(path, apiKey);
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const response = await fetchImpl(url, {
      signal: controller.signal,
      headers: {
        "User-Agent": userAgent,
      },
    });

    if (!response.ok) {
      throw new Error(`API error: ${response.status}`);
    }

    return (await response.json()) as T;
  } catch (error) {
    if (error instanceof Error && error.name === "AbortError") {
      throw new Error("RAWG request timed out");
    }

    if (error instanceof TypeError) {
      throw new Error(`Network error: ${error.message}`);
    }

    if (error instanceof Error) {
      throw new Error(error.message.startsWith("API error:") ? error.message : `Parse error: ${error.message}`);
    }

    throw new Error("Unexpected RAWG request failure");
  } finally {
    clearTimeout(timeout);
  }
}

export function createRawgClient(options: RawgClientOptions): RawgClient {
  const fetchImpl = options.fetchImpl ?? globalThis.fetch;
  if (!fetchImpl) {
    throw new Error("A fetch implementation is required for RAWG requests");
  }

  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const userAgent = options.userAgent ?? DEFAULT_USER_AGENT;

  return {
    async searchRawg(query: string): Promise<RawgGame[]> {
      const apiKey = await options.getApiKey();
      const params = new URLSearchParams({
        search: query,
        page_size: "10",
      });

      const path = `games?${params.toString()}`;
      const result = await requestJson<{ count: number; results: RawgGame[] }>(
        fetchImpl,
        apiKey,
        path,
        timeoutMs,
        userAgent,
      );
      return result.results;
    },

    async getRawgGameDetails(rawgId: number): Promise<RawgGameDetails> {
      const apiKey = await options.getApiKey();
      return await requestJson<RawgGameDetails>(
        fetchImpl,
        apiKey,
        `games/${rawgId}`,
        timeoutMs,
        userAgent,
      );
    },
  };
}

export function buildRawgMetadataSummary(details: RawgGameDetails): RawgMetadataSummary {
  return {
    name: details.name ?? null,
    description: details.description_raw ?? details.description ?? null,
    released: details.released ?? null,
    background_image: details.background_image ?? null,
    metacritic: details.metacritic ?? null,
    rating: details.rating ?? null,
    genres: joinNames(details.genres),
    platforms: joinNames(details.platforms?.map((item) => item.platform) ?? null),
    developers: joinNames(details.developers),
    publishers: joinNames(details.publishers),
  };
}

export function toDeveloperNames(developers: RawgDeveloper[] | null | undefined): string | null {
  return joinNames(developers);
}

export function toPublisherNames(publishers: RawgPublisher[] | null | undefined): string | null {
  return joinNames(publishers);
}
