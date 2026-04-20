<script setup lang="ts">
import {
  Clock,
  Filter,
  Gamepad2,
  Grid3X3,
  List,
  ListFilter,
  Play,
  SortAsc,
  Star,
  X,
} from "lucide-vue-next";
import { computed, onMounted, reactive, shallowRef, watch } from "vue";
import { RouterLink } from "vue-router";
import { gamesApi } from "../../src/lib/api";
import { translateGenreListToRu, translateGenreToRu } from "../../src/lib/genres";
import type { Game, NewGame } from "../../src/types";
import GameCard from "../components/GameCard.vue";
import RawgMetadataPrompt from "../components/RawgMetadataPrompt.vue";
import { useToast } from "../composables/useToast";
import { useDropZone } from "../composables/useDropZone";
import { useGamesStore } from "../stores/games";

type ViewMode = "grid" | "list";
type SortBy = "name" | "lastPlayed" | "dateAdded" | "playCount" | "playtime";
type PlayedState = "all" | "played" | "unplayed";
type RatingMode = "user" | "metacritic";
type PlayStatusState =
  | "all"
  | "not_started"
  | "in_progress"
  | "completed"
  | "abandoned";
type InstallState = "all" | "installed" | "not_installed";

type LibraryFilterPreset = {
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

const FILTER_PRESET_STORAGE_KEY = "arrancador_library_filter_preset_v1";

const defaultLibraryFilterPreset: LibraryFilterPreset = {
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

const sortByLabel: Record<SortBy, string> = {
  name: "По имени",
  lastPlayed: "Недавно запущенные",
  dateAdded: "Недавно добавленные",
  playCount: "По запускам",
  playtime: "По времени",
};

const playedStateLabel: Record<PlayedState, string> = {
  all: "Все",
  played: "Играли",
  unplayed: "Не играли",
};

const installStateLabel: Record<InstallState, string> = {
  all: "Все",
  installed: "Установленные",
  not_installed: "Не установленные",
};

const playStatusLabel: Record<PlayStatusState, string> = {
  all: "Все",
  not_started: "Не начато",
  in_progress: "В процессе",
  completed: "Пройдено",
  abandoned: "Брошено",
};

function createDefaultFilterPreset(): LibraryFilterPreset {
  return {
    ...defaultLibraryFilterPreset,
    selectedGenres: [],
    selectedPlatforms: [],
  };
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

function loadLibraryFilterPreset(): LibraryFilterPreset {
  const fallback = createDefaultFilterPreset();
  if (typeof window === "undefined") return fallback;
  const stored = window.localStorage.getItem(FILTER_PRESET_STORAGE_KEY);
  if (!stored) return fallback;

  try {
    const raw = JSON.parse(stored);
    if (!raw || typeof raw !== "object") return fallback;

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
  } catch {
    return fallback;
  }
}

function persistLibraryFilterPreset(preset: LibraryFilterPreset) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(FILTER_PRESET_STORAGE_KEY, JSON.stringify(preset));
  } catch {
    // Ignore persistence failures.
  }
}

function parseNumber(value: string) {
  if (!value.trim()) return null;
  const parsed = Number.parseFloat(value.replace(",", "."));
  return Number.isFinite(parsed) ? parsed : null;
}

function splitList(value: string | null) {
  return value
    ?.split(",")
    .map((item) => item.trim())
    .filter(Boolean) ?? [];
}

function formatPlaytime(seconds: number) {
  if (!seconds) return "0 ч";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) return `${hours} ч ${minutes} мин`;
  return `${minutes} мин`;
}

function fileNameFromPath(filePath: string) {
  const normalized = filePath.replace(/\\/g, "/");
  const name = normalized.split("/").pop();
  return name || filePath;
}

function cleanNameFromFile(fileName: string) {
  const base = fileName.replace(/\.exe$/i, "");
  return base.replace(/[-_]/g, " ").replace(/\s+/g, " ").trim() || base;
}

function normalizeNameForMerge(value: string) {
  return value
    .normalize("NFKD")
    .toLowerCase()
    .replace(/\.exe$/i, "")
    .replace(/\[[^\]]*\]|\([^\)]*\)|\{[^\}]*\}/g, " ")
    .replace(/[-_./]/g, " ")
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function matchMergeNames(candidate: string, incoming: string) {
  if (!candidate || !incoming) return false;
  if (candidate === incoming) return true;
  if (
    candidate.length > 4 &&
    incoming.length > 4 &&
    (candidate.includes(incoming) || incoming.includes(candidate))
  ) {
    return true;
  }

  const candidateTokens = candidate.split(" ").filter(Boolean);
  const incomingTokens = incoming.split(" ").filter(Boolean);
  if (candidateTokens.length <= 1 || incomingTokens.length <= 1) return false;

  const tokenSet = new Set(incomingTokens);
  const overlap = candidateTokens.filter((token) => tokenSet.has(token)).length;
  if (overlap < 2) return false;

  return overlap / Math.max(candidateTokens.length, incomingTokens.length) >= 0.67;
}

function findMergeCandidate(games: Game[], name: string): Game | undefined {
  const normalized = normalizeNameForMerge(name);
  if (!normalized) return undefined;

  for (const game of games) {
    const candidateNames = [game.name, game.exe_name, fileNameFromPath(game.exe_path)];
    const exact = candidateNames.find(
      (candidateName) => normalizeNameForMerge(candidateName) === normalized,
    );
    if (exact) return game;
  }

  return games.find((game) =>
    [game.name, game.exe_name, fileNameFromPath(game.exe_path)].some(
      (candidateName) => {
        const normalizedCandidate = normalizeNameForMerge(candidateName);
        return matchMergeNames(normalizedCandidate, normalized);
      },
    ),
  );
}

const gamesStore = useGamesStore();
const { notify } = useToast();
const initialFilters = loadLibraryFilterPreset();

const state = reactive({
  searchQuery: initialFilters.searchQuery,
  viewMode: initialFilters.viewMode,
  sortBy: initialFilters.sortBy,
  showFavoritesOnly: initialFilters.showFavoritesOnly,
  showAdvancedFilters: initialFilters.showAdvancedFilters,
  selectedGenres: initialFilters.selectedGenres,
  selectedPlatforms: initialFilters.selectedPlatforms,
  playedState: initialFilters.playedState,
  installState: initialFilters.installState,
  playStatusState: initialFilters.playStatusState,
  ratingMode: initialFilters.ratingMode,
  minRating: initialFilters.minRating,
  maxRating: initialFilters.maxRating,
  minMetacritic: initialFilters.minMetacritic,
  maxMetacritic: initialFilters.maxMetacritic,
  minPlaytimeHours: initialFilters.minPlaytimeHours,
  maxPlaytimeHours: initialFilters.maxPlaytimeHours,
  requireMetadata: initialFilters.requireMetadata,
});

const metadataQueue = shallowRef<Game[]>([]);
const installedById = shallowRef<Record<string, boolean>>({});

function enqueueMetadata(added: Game[]) {
  metadataQueue.value = [
    ...metadataQueue.value,
    ...added.filter(
      (candidate) => !metadataQueue.value.some((queued) => queued.id === candidate.id),
    ),
  ];
}

async function handleDroppedPaths(paths: string[]) {
  if (paths.length === 0) return;

  const toAdd: NewGame[] = [];
  const seen = new Set<string>();
  let skipped = 0;
  let invalid = 0;
  let merged = 0;

  for (const rawPath of paths) {
    let resolved = rawPath;
    if (rawPath.toLowerCase().endsWith(".lnk")) {
      try {
        resolved = await gamesApi.resolveShortcutTarget(rawPath);
      } catch (cause) {
        console.error("Failed to resolve shortcut:", cause);
        invalid += 1;
        continue;
      }
    }

    const lowerResolved = resolved.toLowerCase();
    if (!lowerResolved.endsWith(".exe")) {
      invalid += 1;
      continue;
    }

    if (seen.has(lowerResolved)) {
      continue;
    }

    seen.add(lowerResolved);
    const exists = await gamesApi.existsByPath(resolved).catch(() => false);
    if (exists) {
      skipped += 1;
      continue;
    }

    const exeName = fileNameFromPath(resolved);
    const name = cleanNameFromFile(exeName);
    const candidate = findMergeCandidate(gamesStore.games, name);
    if (candidate) {
      const doMerge = window.confirm(
        `Найдена игра с похожим названием: "${candidate.name}" из библиотеки. Обновить путь для существующей записи?`,
      );

      if (doMerge) {
        await gamesStore.updateGame(candidate.id, { exe_path: resolved });
        merged += 1;
        continue;
      }
    }

    toAdd.push({
      name,
      exe_path: resolved,
      exe_name: exeName,
    });
  }

  if (toAdd.length > 0) {
    try {
      const added = await gamesStore.addGames(toAdd);
      enqueueMetadata(added);

      const extra: string[] = [];
      if (skipped > 0) extra.push(`Пропущено: ${skipped}`);
      if (invalid > 0) extra.push(`Не поддерживается: ${invalid}`);
      if (merged > 0) extra.push(`Объединено с существующими: ${merged}`);

      notify({
        tone: "success",
        title: `Добавлено ${added.length}`,
        description: extra.length ? extra.join(" | ") : undefined,
      });
    } catch (cause) {
      console.error("Failed to add dropped games:", cause);
      notify({
        tone: "error",
        title: "Не удалось добавить игру",
        description: "Проверьте путь к файлу.",
      });
    }
    return;
  }

  if (merged > 0) {
    notify({
      tone: "success",
      title: `Объединено с существующими: ${merged}`,
      description: "Обновлены пути для игр с совпавшим названием.",
    });
    return;
  }

  notify({
    tone: "warning",
    title: "Файлы не добавлены",
    description: "Поддерживаются .exe и .lnk.",
  });
}

const { dropActive, dropHandlers } = useDropZone({
  onDropPaths: handleDroppedPaths,
});

const genreOptions = computed(() => {
  const values = new Set<string>();
  for (const game of gamesStore.games) {
    for (const genre of splitList(game.genres)) {
      values.add(genre);
    }
  }
  return Array.from(values).sort((left, right) => left.localeCompare(right));
});

const platformOptions = computed(() => {
  const values = new Set<string>();
  for (const game of gamesStore.games) {
    for (const platform of splitList(game.platforms)) {
      values.add(platform);
    }
  }
  return Array.from(values).sort((left, right) => left.localeCompare(right));
});

function clearAdvancedFilters() {
  Object.assign(state, createDefaultFilterPreset());
}

watch(
  () => ({ ...state, selectedGenres: [...state.selectedGenres], selectedPlatforms: [...state.selectedPlatforms] }),
  (next) => {
    persistLibraryFilterPreset({
      ...next,
    });
  },
  { deep: true },
);

watch(
  () => gamesStore.games,
  async (games) => {
    if (games.length === 0) {
      installedById.value = {};
      return;
    }

    const resolved = await Promise.all(
      games.map(async (game) => {
        try {
          const value = await gamesApi.isInstalled(game.id);
          return [game.id, value] as const;
        } catch (cause) {
          console.error("Failed to check install status:", cause);
          return [game.id, true] as const;
        }
      }),
    );

    installedById.value = Object.fromEntries(resolved);
  },
  { immediate: true, deep: true },
);

const hasActiveAdvancedFilters = computed(() => {
  const minRatingValue = parseNumber(state.minRating);
  const maxRatingValue = parseNumber(state.maxRating);
  const minMetacriticValue = parseNumber(state.minMetacritic);
  const maxMetacriticValue = parseNumber(state.maxMetacritic);
  const minPlaytimeValue = parseNumber(state.minPlaytimeHours);
  const maxPlaytimeValue = parseNumber(state.maxPlaytimeHours);

  return (
    state.showFavoritesOnly ||
    state.selectedGenres.length > 0 ||
    state.selectedPlatforms.length > 0 ||
    state.playedState !== "all" ||
    state.installState !== "all" ||
    state.playStatusState !== "all" ||
    state.requireMetadata ||
    minRatingValue !== null ||
    maxRatingValue !== null ||
    minMetacriticValue !== null ||
    maxMetacriticValue !== null ||
    minPlaytimeValue !== null ||
    maxPlaytimeValue !== null
  );
});

const activeFilterCount = computed(() => {
  const minRatingValue = parseNumber(state.minRating);
  const maxRatingValue = parseNumber(state.maxRating);
  const minMetacriticValue = parseNumber(state.minMetacritic);
  const maxMetacriticValue = parseNumber(state.maxMetacritic);
  const minPlaytimeValue = parseNumber(state.minPlaytimeHours);
  const maxPlaytimeValue = parseNumber(state.maxPlaytimeHours);

  return (
    (state.showFavoritesOnly ? 1 : 0) +
    state.selectedGenres.length +
    state.selectedPlatforms.length +
    (state.playedState !== "all" ? 1 : 0) +
    (state.installState !== "all" ? 1 : 0) +
    (state.playStatusState !== "all" ? 1 : 0) +
    (state.requireMetadata ? 1 : 0) +
    (minRatingValue !== null ? 1 : 0) +
    (maxRatingValue !== null ? 1 : 0) +
    (minMetacriticValue !== null ? 1 : 0) +
    (maxMetacriticValue !== null ? 1 : 0) +
    (minPlaytimeValue !== null ? 1 : 0) +
    (maxPlaytimeValue !== null ? 1 : 0)
  );
});

const filteredGames = computed(() => {
  let items = [...gamesStore.games];
  const searchNeedle = state.searchQuery.trim().toLowerCase();

  if (state.showFavoritesOnly) {
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

  if (state.selectedGenres.length > 0) {
    items = items.filter((game) => {
      const genres = splitList(game.genres);
      return state.selectedGenres.every((genre) => genres.includes(genre));
    });
  }

  if (state.selectedPlatforms.length > 0) {
    items = items.filter((game) => {
      const platforms = splitList(game.platforms);
      return state.selectedPlatforms.every((platform) => platforms.includes(platform));
    });
  }

  if (state.playedState === "played") {
    items = items.filter((game) => game.total_playtime > 0 || game.play_count > 0);
  } else if (state.playedState === "unplayed") {
    items = items.filter((game) => game.total_playtime <= 0 && game.play_count <= 0);
  }

  if (state.installState === "installed") {
    items = items.filter((game) => installedById.value[game.id] !== false);
  } else if (state.installState === "not_installed") {
    items = items.filter((game) => installedById.value[game.id] === false);
  }

  if (state.playStatusState !== "all") {
    items = items.filter((game) => game.play_status === state.playStatusState);
  }

  if (state.requireMetadata) {
    items = items.filter((game) => Boolean(game.rawg_id || game.description || game.background_image));
  }

  const minRatingValue = parseNumber(state.minRating);
  const maxRatingValue = parseNumber(state.maxRating);
  const minMetacriticValue = parseNumber(state.minMetacritic);
  const maxMetacriticValue = parseNumber(state.maxMetacritic);
  const minPlaytimeValue = parseNumber(state.minPlaytimeHours);
  const maxPlaytimeValue = parseNumber(state.maxPlaytimeHours);

  if (minRatingValue !== null || maxRatingValue !== null) {
    items = items.filter((game) => {
      const ratingValue =
        state.ratingMode === "user" ? game.user_rating : game.metacritic;
      if (ratingValue == null) return false;
      if (minRatingValue !== null && ratingValue < minRatingValue) return false;
      if (maxRatingValue !== null && ratingValue > maxRatingValue) return false;
      return true;
    });
  }

  if (minMetacriticValue !== null || maxMetacriticValue !== null) {
    items = items.filter((game) => {
      if (game.metacritic == null) return false;
      if (minMetacriticValue !== null && game.metacritic < minMetacriticValue) return false;
      if (maxMetacriticValue !== null && game.metacritic > maxMetacriticValue) return false;
      return true;
    });
  }

  if (minPlaytimeValue !== null || maxPlaytimeValue !== null) {
    items = items.filter((game) => {
      const hours = game.total_playtime / 3600;
      if (minPlaytimeValue !== null && hours < minPlaytimeValue) return false;
      if (maxPlaytimeValue !== null && hours > maxPlaytimeValue) return false;
      return true;
    });
  }

  items.sort((left, right) => {
    switch (state.sortBy) {
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
      case "name":
      default:
        return left.name.localeCompare(right.name);
    }
  });

  return items;
});

const currentMetadataGame = computed(() => metadataQueue.value[0] ?? null);

onMounted(async () => {
  if (gamesStore.games.length === 0) {
    await gamesStore.refreshGames();
  }
});
</script>

<template>
  <div
    class="flex h-full w-full flex-col p-4 sm:p-6"
    @dragenter="dropHandlers.onDragenter"
    @dragover="dropHandlers.onDragover"
    @dragleave="dropHandlers.onDragleave"
    @drop="dropHandlers.onDrop"
  >
    <div class="mb-4 flex flex-col gap-4 sm:mb-6">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h1 class="text-xl font-bold tracking-tight sm:text-2xl">Библиотека</h1>
          <p class="text-xs text-muted-foreground sm:text-sm">
            {{ gamesStore.games.length }} игр · {{ gamesStore.favorites.length }} в избранном
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-2">
          <button
            type="button"
            class="inline-flex h-9 items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 text-sm transition-colors hover:bg-accent/70"
            @click="state.showAdvancedFilters = !state.showAdvancedFilters"
          >
            <Filter class="h-4 w-4" />
            <span>Фильтры</span>
            <span
              v-if="activeFilterCount > 0"
              class="rounded-full bg-primary px-2 py-0.5 text-[11px] text-primary-foreground"
            >
              {{ activeFilterCount }}
            </span>
          </button>

          <div class="inline-flex rounded-xl border border-border/70 bg-card/80 p-1">
            <button
              type="button"
              class="inline-flex h-8 w-8 items-center justify-center rounded-lg transition-colors"
              :class="state.viewMode === 'grid' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground'"
              @click="state.viewMode = 'grid'"
            >
              <Grid3X3 class="h-4 w-4" />
            </button>
            <button
              type="button"
              class="inline-flex h-8 w-8 items-center justify-center rounded-lg transition-colors"
              :class="state.viewMode === 'list' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground'"
              @click="state.viewMode = 'list'"
            >
              <List class="h-4 w-4" />
            </button>
          </div>
        </div>
      </div>

      <div class="flex flex-col gap-3 xl:flex-row xl:items-center">
        <div class="relative flex-1">
          <input
            v-model="state.searchQuery"
            placeholder="Поиск по библиотеке..."
            class="h-11 w-full rounded-xl border border-border/70 bg-card/70 px-4 pr-10 text-sm outline-none"
          />
          <button
            v-if="state.searchQuery"
            type="button"
            class="absolute top-1/2 right-3 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            @click="state.searchQuery = ''"
          >
            <X class="h-4 w-4" />
          </button>
        </div>

        <label class="flex items-center gap-2 text-sm text-muted-foreground">
          <SortAsc class="h-4 w-4" />
          <select
            v-model="state.sortBy"
            class="h-11 rounded-xl border border-border/70 bg-card/80 px-3 text-sm text-foreground outline-none"
          >
            <option v-for="(label, value) in sortByLabel" :key="value" :value="value">
              {{ label }}
            </option>
          </select>
        </label>
      </div>

      <div
        v-if="state.showAdvancedFilters"
        class="rounded-xl border border-border/70 bg-card/40 p-4"
      >
        <div class="mb-4 flex flex-wrap items-center gap-2">
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border px-3 py-2 text-sm transition-colors"
            :class="
              state.showFavoritesOnly
                ? 'border-primary bg-primary text-primary-foreground'
                : 'border-border/70 bg-background/50 hover:bg-accent/60'
            "
            @click="state.showFavoritesOnly = !state.showFavoritesOnly"
          >
            <Star class="h-4 w-4" :class="{ 'fill-current': state.showFavoritesOnly }" />
            Избранное
          </button>

          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-3 py-2 text-sm transition-colors hover:bg-accent/60"
            @click="clearAdvancedFilters()"
          >
            <X class="h-4 w-4" />
            Сбросить
          </button>
        </div>

        <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
          <label class="space-y-1 text-sm">
            <div class="text-xs text-muted-foreground">Жанры</div>
            <select
              multiple
              class="min-h-28 w-full rounded-xl border border-border/70 bg-background/50 px-3 py-2 text-sm outline-none"
              :value="state.selectedGenres"
              @change="
                state.selectedGenres = Array.from(
                  ($event.target as HTMLSelectElement).selectedOptions,
                ).map((option) => option.value)
              "
            >
              <option v-for="genre in genreOptions" :key="genre" :value="genre">
                {{ translateGenreToRu(genre) }}
              </option>
            </select>
          </label>

          <label class="space-y-1 text-sm">
            <div class="text-xs text-muted-foreground">Платформы</div>
            <select
              multiple
              class="min-h-28 w-full rounded-xl border border-border/70 bg-background/50 px-3 py-2 text-sm outline-none"
              :value="state.selectedPlatforms"
              @change="
                state.selectedPlatforms = Array.from(
                  ($event.target as HTMLSelectElement).selectedOptions,
                ).map((option) => option.value)
              "
            >
              <option v-for="platform in platformOptions" :key="platform" :value="platform">
                {{ platform }}
              </option>
            </select>
          </label>

          <div class="space-y-3">
            <label class="space-y-1 text-sm">
              <div class="text-xs text-muted-foreground">Статус игры</div>
              <select
                v-model="state.playedState"
                class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
              >
                <option v-for="(label, value) in playedStateLabel" :key="value" :value="value">
                  {{ label }}
                </option>
              </select>
            </label>

            <label class="space-y-1 text-sm">
              <div class="text-xs text-muted-foreground">Установка</div>
              <select
                v-model="state.installState"
                class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
              >
                <option v-for="(label, value) in installStateLabel" :key="value" :value="value">
                  {{ label }}
                </option>
              </select>
            </label>

            <label class="space-y-1 text-sm">
              <div class="text-xs text-muted-foreground">Прохождение</div>
              <select
                v-model="state.playStatusState"
                class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
              >
                <option v-for="(label, value) in playStatusLabel" :key="value" :value="value">
                  {{ label }}
                </option>
              </select>
            </label>
          </div>

          <div class="space-y-3">
            <label class="space-y-1 text-sm">
              <div class="text-xs text-muted-foreground">Рейтинг</div>
              <div class="flex gap-2">
                <input
                  v-model="state.minRating"
                  inputmode="decimal"
                  placeholder="Мин."
                  class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
                />
                <input
                  v-model="state.maxRating"
                  inputmode="decimal"
                  placeholder="Макс."
                  class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
                />
              </div>
            </label>

            <label class="space-y-1 text-sm">
              <div class="text-xs text-muted-foreground">Источник рейтинга</div>
              <select
                v-model="state.ratingMode"
                class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
              >
                <option value="user">Личный рейтинг</option>
                <option value="metacritic">Metacritic</option>
              </select>
            </label>

            <label class="space-y-1 text-sm">
              <div class="text-xs text-muted-foreground">Metacritic</div>
              <div class="flex gap-2">
                <input
                  v-model="state.minMetacritic"
                  inputmode="decimal"
                  placeholder="Мин."
                  class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
                />
                <input
                  v-model="state.maxMetacritic"
                  inputmode="decimal"
                  placeholder="Макс."
                  class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
                />
              </div>
            </label>

            <label class="space-y-1 text-sm">
              <div class="text-xs text-muted-foreground">Время в игре (часы)</div>
              <div class="flex gap-2">
                <input
                  v-model="state.minPlaytimeHours"
                  inputmode="decimal"
                  placeholder="Мин."
                  class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
                />
                <input
                  v-model="state.maxPlaytimeHours"
                  inputmode="decimal"
                  placeholder="Макс."
                  class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
                />
              </div>
            </label>
          </div>
        </div>

        <div class="mt-4">
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border px-3 py-2 text-sm transition-colors"
            :class="
              state.requireMetadata
                ? 'border-primary bg-primary text-primary-foreground'
                : 'border-border/70 bg-background/50 hover:bg-accent/60'
            "
            @click="state.requireMetadata = !state.requireMetadata"
          >
            <ListFilter class="h-4 w-4" />
            {{ state.requireMetadata ? "Только с метаданными" : "Нужны метаданные" }}
          </button>
        </div>
      </div>
    </div>

    <div
      v-if="filteredGames.length === 0"
      class="flex flex-1 flex-col items-center justify-center py-16 text-center"
    >
      <Gamepad2 class="mb-4 h-12 w-12 text-muted-foreground" />
      <template v-if="gamesStore.games.length === 0">
        <h3 class="mb-2 text-lg font-medium">Нет игр</h3>
        <p class="mb-4 text-muted-foreground">Просканируйте папку, чтобы добавить игры</p>
        <RouterLink
          to="/scan"
          class="inline-flex h-10 items-center rounded-xl bg-primary px-4 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white"
        >
          Сканировать
        </RouterLink>
      </template>
      <template v-else>
        <h3 class="mb-2 text-lg font-medium">Игры не найдены</h3>
        <p class="text-muted-foreground">Попробуйте изменить поиск или фильтры</p>
      </template>
    </div>

    <div v-else-if="state.viewMode === 'grid'" class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
      <GameCard v-for="game in filteredGames" :key="game.id" :game="game" />
    </div>

    <div v-else class="space-y-2">
      <RouterLink
        v-for="game in filteredGames"
        :key="game.id"
        :to="`/game/${game.id}`"
        class="flex items-center gap-3 rounded-lg p-2 transition-colors hover:bg-accent sm:gap-4 sm:p-3"
      >
        <div class="flex h-12 w-12 flex-shrink-0 items-center justify-center overflow-hidden rounded-md bg-muted sm:h-16 sm:w-16">
          <img
            v-if="game.background_image"
            :src="game.background_image"
            :alt="game.name"
            class="h-full w-full object-cover"
          />
          <Gamepad2 v-else class="h-6 w-6 text-muted-foreground" />
        </div>

        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <h3 class="truncate text-sm font-medium sm:text-base">{{ game.name }}</h3>
            <Star
              v-if="game.is_favorite"
              class="h-3 w-3 flex-shrink-0 fill-yellow-500 text-yellow-500 sm:h-4 sm:w-4"
            />
          </div>
          <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground sm:text-sm">
            <span v-if="game.genres" class="truncate">
              {{ translateGenreListToRu(game.genres, 1)[0] }}
            </span>
            <span v-if="game.total_playtime > 0" class="flex items-center gap-1">
              <Clock class="h-3 w-3" />
              {{ formatPlaytime(game.total_playtime) }}
            </span>
            <span v-if="game.last_played" class="hidden items-center gap-1 sm:flex">
              <Play class="h-3 w-3" />
              {{ new Date(game.last_played).toLocaleDateString("ru-RU") }}
            </span>
          </div>
        </div>

        <div
          v-if="game.metacritic != null"
          class="rounded px-1.5 py-0.5 text-[10px] font-medium sm:px-2 sm:py-1 sm:text-sm"
          :class="
            game.metacritic >= 75
              ? 'bg-green-500/10 text-green-500'
              : game.metacritic >= 50
                ? 'bg-yellow-500/10 text-yellow-500'
                : 'bg-red-500/10 text-red-500'
          "
        >
          {{ game.metacritic }}
        </div>
      </RouterLink>
    </div>

    <RawgMetadataPrompt
      v-if="currentMetadataGame"
      :game="currentMetadataGame"
      :remaining="metadataQueue.length"
      @next="metadataQueue = metadataQueue.slice(1)"
      @skip-all="metadataQueue = []"
      @after-apply="void gamesStore.refreshGames()"
    />

    <div v-if="dropActive" class="pointer-events-none fixed inset-0 z-50">
      <div class="absolute inset-0 bg-background/40 backdrop-blur-sm" />
      <div class="absolute inset-3 rounded-2xl border-2 border-dashed border-primary/70 shadow-[0_0_0_1px_rgba(255,255,255,0.06),0_30px_80px_rgba(8,12,24,0.55)] sm:inset-6">
        <div class="absolute inset-0 flex items-center justify-center">
          <div class="rounded-2xl border border-border/60 bg-card/80 px-6 py-4 text-center shadow-[0_18px_45px_rgba(8,12,24,0.45)] backdrop-blur-xl">
            <div class="text-sm font-semibold text-foreground">Отпустите, чтобы добавить игру</div>
            <div class="text-xs text-muted-foreground">Поддерживаются .exe и .lnk</div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
