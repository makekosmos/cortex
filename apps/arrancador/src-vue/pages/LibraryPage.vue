<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import { translateGenreToRu } from "../../src/lib/genres";
import LibraryDropOverlay from "../components/library/LibraryDropOverlay.vue";
import LibraryFiltersPanel from "../components/library/LibraryFiltersPanel.vue";
import LibraryResults from "../components/library/LibraryResults.vue";
import LibraryToolbar from "../components/library/LibraryToolbar.vue";
import RawgMetadataPrompt from "../components/RawgMetadataPrompt.vue";
import { useLibraryGameImport } from "../composables/useLibraryGameImport";
import { useLibraryInstallStatus } from "../composables/useLibraryInstallStatus";
import { useToast } from "../composables/useToast";
import {
  countActiveLibraryFilters,
  createDefaultLibraryFilterPreset,
  filterLibraryGames,
  getLibraryMetadataOptions,
  type InstallState,
  type LibraryFilterPreset,
  type PlayedState,
  type PlayStatusState,
  type SortBy,
  sanitizeLibraryFilterPreset,
} from "../lib/libraryFilters";
import { useGamesStore } from "../stores/games";

const FILTER_PRESET_STORAGE_KEY = "arrancador_library_filter_preset_v1";

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

function loadLibraryFilterPreset(): LibraryFilterPreset {
  const fallback = createDefaultLibraryFilterPreset();
  if (typeof window === "undefined") return fallback;
  const stored = window.localStorage.getItem(FILTER_PRESET_STORAGE_KEY);
  if (!stored) return fallback;

  try {
    return sanitizeLibraryFilterPreset(JSON.parse(stored));
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

const {
  metadataQueue,
  currentMetadataGame,
  dropActive,
  dropHandlers,
  advanceMetadataQueue,
  clearMetadataQueue,
} = useLibraryGameImport({
  games: () => gamesStore.games,
  addGames: gamesStore.addGames,
  updateGame: gamesStore.updateGame,
  notify,
});

const { installedById } = useLibraryInstallStatus({
  games: () => gamesStore.games,
});

const genreOptions = computed(() => getLibraryMetadataOptions(gamesStore.games, "genres"));

const platformOptions = computed(() => getLibraryMetadataOptions(gamesStore.games, "platforms"));

function clearAdvancedFilters() {
  Object.assign(state, createDefaultLibraryFilterPreset());
}

watch(
  () => ({
    ...state,
    selectedGenres: [...state.selectedGenres],
    selectedPlatforms: [...state.selectedPlatforms],
  }),
  (next) => {
    persistLibraryFilterPreset({
      ...next,
    });
  },
  { deep: true },
);

const activeFilterCount = computed(() => countActiveLibraryFilters(state));

const filteredGames = computed(() =>
  filterLibraryGames(gamesStore.games, state, installedById.value),
);
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
      <LibraryToolbar
        v-model:search-query="state.searchQuery"
        v-model:view-mode="state.viewMode"
        v-model:sort-by="state.sortBy"
        v-model:show-advanced-filters="state.showAdvancedFilters"
        :total-games="gamesStore.games.length"
        :favorite-count="gamesStore.favorites.length"
        :active-filter-count="activeFilterCount"
        :sort-by-label="sortByLabel"
      />

      <LibraryFiltersPanel
        v-if="state.showAdvancedFilters"
        v-model:show-favorites-only="state.showFavoritesOnly"
        v-model:selected-genres="state.selectedGenres"
        v-model:selected-platforms="state.selectedPlatforms"
        v-model:played-state="state.playedState"
        v-model:install-state="state.installState"
        v-model:play-status-state="state.playStatusState"
        v-model:rating-mode="state.ratingMode"
        v-model:min-rating="state.minRating"
        v-model:max-rating="state.maxRating"
        v-model:min-metacritic="state.minMetacritic"
        v-model:max-metacritic="state.maxMetacritic"
        v-model:min-playtime-hours="state.minPlaytimeHours"
        v-model:max-playtime-hours="state.maxPlaytimeHours"
        v-model:require-metadata="state.requireMetadata"
        :genre-options="genreOptions"
        :platform-options="platformOptions"
        :played-state-label="playedStateLabel"
        :install-state-label="installStateLabel"
        :play-status-label="playStatusLabel"
        :translate-genre="translateGenreToRu"
        @clear-filters="clearAdvancedFilters"
      />
    </div>

    <LibraryResults
      :games="filteredGames"
      :total-game-count="gamesStore.games.length"
      :view-mode="state.viewMode"
    />

    <RawgMetadataPrompt
      v-if="currentMetadataGame"
      :game="currentMetadataGame"
      :remaining="metadataQueue.length"
      @next="advanceMetadataQueue"
      @skip-all="clearMetadataQueue"
      @after-apply="void gamesStore.refreshGames()"
    />

    <LibraryDropOverlay v-if="dropActive" />
  </div>
</template>
