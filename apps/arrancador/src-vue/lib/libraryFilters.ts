import type { Game } from "../../src/types";

export type ViewMode = "grid" | "list";
export type SortBy = "name" | "lastPlayed" | "dateAdded" | "playCount" | "playtime";
export type PlayedState = "all" | "played" | "unplayed";
export type RatingMode = "user" | "metacritic";
export type PlayStatusState =
  | "all"
  | "not_started"
  | "in_progress"
  | "completed"
  | "abandoned";
export type InstallState = "all" | "installed" | "not_installed";

export type LibraryFilterPreset = {
  searchQuery: string;
  viewMode: ViewMode;
  sortBy: SortBy;
  showFavoritesOnly: boolean;
  showAdvancedFilters: boolean;
  selectedGenres: string[];
  selectedPlatforms: string[];
  playedState: PlayedState;
  installState: InstallState;
  playStatusState: PlayStatusState;
  ratingMode: RatingMode;
  minRating: string;
  maxRating: string;
  minMetacritic: string;
  maxMetacritic: string;
  minPlaytimeHours: string;
  maxPlaytimeHours: string;
  requireMetadata: boolean;
};

export const defaultLibraryFilterPreset: LibraryFilterPreset = {
  searchQuery: "",
  viewMode: "grid",
  sortBy: "name",
  showFavoritesOnly: false,
  showAdvancedFilters: false,
  selectedGenres: [],
  selectedPlatforms: [],
  playedState: "all",
  installState: "all",
  playStatusState: "all",
  ratingMode: "user",
  minRating: "",
  maxRating: "",
  minMetacritic: "",
  maxMetacritic: "",
  minPlaytimeHours: "",
  maxPlaytimeHours: "",
  requireMetadata: false,
};

export function createDefaultLibraryFilterPreset(): LibraryFilterPreset {
  return {
    ...defaultLibraryFilterPreset,
    selectedGenres: [],
    selectedPlatforms: [],
  };
}

export function parseNumber(value: string) {
  if (!value.trim()) return null;
  const parsed = Number.parseFloat(value.replace(",", "."));
  return Number.isFinite(parsed) ? parsed : null;
}

export function splitList(value: string | null) {
  return value
    ?.split(",")
    .map((item) => item.trim())
    .filter(Boolean) ?? [];
}

export function formatPlaytime(seconds: number) {
  if (!seconds) return "0 ч";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) return `${hours} ч ${minutes} мин`;
  return `${minutes} мин`;
}

function isViewMode(value: unknown): value is ViewMode {
  return value === "grid" || value === "list";
}

function isSortBy(value: unknown): value is SortBy {
  return (
    value === "name" ||
    value === "lastPlayed" ||
    value === "dateAdded" ||
    value === "playCount" ||
    value === "playtime"
  );
}

function isPlayedState(value: unknown): value is PlayedState {
  return value === "all" || value === "played" || value === "unplayed";
}

function isInstallState(value: unknown): value is InstallState {
  return value === "all" || value === "installed" || value === "not_installed";
}

function isPlayStatusState(value: unknown): value is PlayStatusState {
  return (
    value === "all" ||
    value === "not_started" ||
    value === "in_progress" ||
    value === "completed" ||
    value === "abandoned"
  );
}

function isRatingMode(value: unknown): value is RatingMode {
  return value === "user" || value === "metacritic";
}

function sanitizeStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((item) => typeof item === "string");
}

export function sanitizeLibraryFilterPreset(value: unknown): LibraryFilterPreset {
  const fallback = createDefaultLibraryFilterPreset();
  if (!value || typeof value !== "object") return fallback;
  const raw = value as Record<string, unknown>;

  return {
    ...fallback,
    searchQuery:
      typeof raw.searchQuery === "string" ? raw.searchQuery : fallback.searchQuery,
    viewMode: isViewMode(raw.viewMode) ? raw.viewMode : fallback.viewMode,
    sortBy: isSortBy(raw.sortBy) ? raw.sortBy : fallback.sortBy,
    showFavoritesOnly:
      typeof raw.showFavoritesOnly === "boolean"
        ? raw.showFavoritesOnly
        : fallback.showFavoritesOnly,
    showAdvancedFilters:
      typeof raw.showAdvancedFilters === "boolean"
        ? raw.showAdvancedFilters
        : fallback.showAdvancedFilters,
    selectedGenres: sanitizeStringArray(raw.selectedGenres),
    selectedPlatforms: sanitizeStringArray(raw.selectedPlatforms),
    playedState: isPlayedState(raw.playedState) ? raw.playedState : fallback.playedState,
    installState: isInstallState(raw.installState)
      ? raw.installState
      : fallback.installState,
    playStatusState: isPlayStatusState(raw.playStatusState)
      ? raw.playStatusState
      : fallback.playStatusState,
    ratingMode: isRatingMode(raw.ratingMode) ? raw.ratingMode : fallback.ratingMode,
    minRating: typeof raw.minRating === "string" ? raw.minRating : fallback.minRating,
    maxRating: typeof raw.maxRating === "string" ? raw.maxRating : fallback.maxRating,
    minMetacritic:
      typeof raw.minMetacritic === "string"
        ? raw.minMetacritic
        : fallback.minMetacritic,
    maxMetacritic:
      typeof raw.maxMetacritic === "string"
        ? raw.maxMetacritic
        : fallback.maxMetacritic,
    minPlaytimeHours:
      typeof raw.minPlaytimeHours === "string"
        ? raw.minPlaytimeHours
        : fallback.minPlaytimeHours,
    maxPlaytimeHours:
      typeof raw.maxPlaytimeHours === "string"
        ? raw.maxPlaytimeHours
        : fallback.maxPlaytimeHours,
    requireMetadata:
      typeof raw.requireMetadata === "boolean"
        ? raw.requireMetadata
        : fallback.requireMetadata,
  };
}

type NumericFilters = {
  minRatingValue: number | null;
  maxRatingValue: number | null;
  minMetacriticValue: number | null;
  maxMetacriticValue: number | null;
  minPlaytimeValue: number | null;
  maxPlaytimeValue: number | null;
};

function readNumericFilters(filters: LibraryFilterPreset): NumericFilters {
  return {
    minRatingValue: parseNumber(filters.minRating),
    maxRatingValue: parseNumber(filters.maxRating),
    minMetacriticValue: parseNumber(filters.minMetacritic),
    maxMetacriticValue: parseNumber(filters.maxMetacritic),
    minPlaytimeValue: parseNumber(filters.minPlaytimeHours),
    maxPlaytimeValue: parseNumber(filters.maxPlaytimeHours),
  };
}

export function hasActiveLibraryFilters(filters: LibraryFilterPreset) {
  const numeric = readNumericFilters(filters);
  return (
    filters.showFavoritesOnly ||
    filters.selectedGenres.length > 0 ||
    filters.selectedPlatforms.length > 0 ||
    filters.playedState !== "all" ||
    filters.installState !== "all" ||
    filters.playStatusState !== "all" ||
    filters.requireMetadata ||
    numeric.minRatingValue !== null ||
    numeric.maxRatingValue !== null ||
    numeric.minMetacriticValue !== null ||
    numeric.maxMetacriticValue !== null ||
    numeric.minPlaytimeValue !== null ||
    numeric.maxPlaytimeValue !== null
  );
}

export function countActiveLibraryFilters(filters: LibraryFilterPreset) {
  const numeric = readNumericFilters(filters);
  return (
    (filters.showFavoritesOnly ? 1 : 0) +
    filters.selectedGenres.length +
    filters.selectedPlatforms.length +
    (filters.playedState !== "all" ? 1 : 0) +
    (filters.installState !== "all" ? 1 : 0) +
    (filters.playStatusState !== "all" ? 1 : 0) +
    (filters.requireMetadata ? 1 : 0) +
    (numeric.minRatingValue !== null ? 1 : 0) +
    (numeric.maxRatingValue !== null ? 1 : 0) +
    (numeric.minMetacriticValue !== null ? 1 : 0) +
    (numeric.maxMetacriticValue !== null ? 1 : 0) +
    (numeric.minPlaytimeValue !== null ? 1 : 0) +
    (numeric.maxPlaytimeValue !== null ? 1 : 0)
  );
}

export function getLibraryMetadataOptions(
  games: readonly Game[],
  field: "genres" | "platforms",
) {
  const values = new Set<string>();
  for (const game of games) {
    for (const item of splitList(game[field])) {
      values.add(item);
    }
  }
  return Array.from(values).sort((left, right) => left.localeCompare(right));
}

export function filterLibraryGames(
  games: readonly Game[],
  filters: LibraryFilterPreset,
  installedById: Record<string, boolean>,
) {
  let items = [...games];
  const searchNeedle = filters.searchQuery.trim().toLowerCase();

  if (filters.showFavoritesOnly) {
    items = items.filter((game) => game.is_favorite);
  }

  if (searchNeedle) {
    items = items.filter((game) =>
      [
        game.name,
        game.exe_name,
        game.description ?? "",
        game.genres ?? "",
        game.platforms ?? "",
        game.developers ?? "",
        game.publishers ?? "",
      ]
        .join(" ")
        .toLowerCase()
        .includes(searchNeedle),
    );
  }

  if (filters.selectedGenres.length > 0) {
    items = items.filter((game) => {
      const genres = splitList(game.genres);
      return filters.selectedGenres.every((genre) => genres.includes(genre));
    });
  }

  if (filters.selectedPlatforms.length > 0) {
    items = items.filter((game) => {
      const platforms = splitList(game.platforms);
      return filters.selectedPlatforms.every((platform) => platforms.includes(platform));
    });
  }

  if (filters.playedState === "played") {
    items = items.filter((game) => game.total_playtime > 0 || game.play_count > 0);
  } else if (filters.playedState === "unplayed") {
    items = items.filter((game) => game.total_playtime <= 0 && game.play_count <= 0);
  }

  if (filters.installState === "installed") {
    items = items.filter((game) => installedById[game.id] !== false);
  } else if (filters.installState === "not_installed") {
    items = items.filter((game) => installedById[game.id] === false);
  }

  if (filters.playStatusState !== "all") {
    items = items.filter((game) => game.play_status === filters.playStatusState);
  }

  if (filters.requireMetadata) {
    items = items.filter((game) =>
      Boolean(game.rawg_id || game.description || game.background_image),
    );
  }

  const numeric = readNumericFilters(filters);

  if (numeric.minRatingValue !== null || numeric.maxRatingValue !== null) {
    items = items.filter((game) => {
      const ratingValue =
        filters.ratingMode === "user" ? game.user_rating : game.metacritic;
      if (ratingValue == null) return false;
      if (numeric.minRatingValue !== null && ratingValue < numeric.minRatingValue) {
        return false;
      }
      if (numeric.maxRatingValue !== null && ratingValue > numeric.maxRatingValue) {
        return false;
      }
      return true;
    });
  }

  if (numeric.minMetacriticValue !== null || numeric.maxMetacriticValue !== null) {
    items = items.filter((game) => {
      if (game.metacritic == null) return false;
      if (
        numeric.minMetacriticValue !== null &&
        game.metacritic < numeric.minMetacriticValue
      ) {
        return false;
      }
      if (
        numeric.maxMetacriticValue !== null &&
        game.metacritic > numeric.maxMetacriticValue
      ) {
        return false;
      }
      return true;
    });
  }

  if (numeric.minPlaytimeValue !== null || numeric.maxPlaytimeValue !== null) {
    items = items.filter((game) => {
      const hours = game.total_playtime / 3600;
      if (numeric.minPlaytimeValue !== null && hours < numeric.minPlaytimeValue) {
        return false;
      }
      if (numeric.maxPlaytimeValue !== null && hours > numeric.maxPlaytimeValue) {
        return false;
      }
      return true;
    });
  }

  items.sort((left, right) => {
    switch (filters.sortBy) {
      case "lastPlayed": {
        const leftValue = left.last_played ? new Date(left.last_played).getTime() : 0;
        const rightValue = right.last_played ? new Date(right.last_played).getTime() : 0;
        return rightValue - leftValue;
      }
      case "dateAdded":
        return new Date(right.date_added).getTime() - new Date(left.date_added).getTime();
      case "playCount":
        return right.play_count - left.play_count;
      case "playtime":
        return right.total_playtime - left.total_playtime;
      default:
        return left.name.localeCompare(right.name);
    }
  });

  return items;
}
