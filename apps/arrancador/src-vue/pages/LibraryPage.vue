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
import { computed, reactive, watch } from "vue";
import { RouterLink } from "vue-router";
import { translateGenreListToRu, translateGenreToRu } from "../../src/lib/genres";
import GameCard from "../components/GameCard.vue";
import RawgMetadataPrompt from "../components/RawgMetadataPrompt.vue";
import { useLibraryGameImport } from "../composables/useLibraryGameImport";
import { useLibraryInstallStatus } from "../composables/useLibraryInstallStatus";
import { useToast } from "../composables/useToast";
import {
  countActiveLibraryFilters,
  createDefaultLibraryFilterPreset,
  filterLibraryGames,
  formatPlaytime,
  getLibraryMetadataOptions,
  hasActiveLibraryFilters,
  type InstallState,
  type LibraryFilterPreset,
  type PlayedState,
  type PlayStatusState,
  type RatingMode,
  type SortBy,
  sanitizeLibraryFilterPreset,
  type ViewMode,
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

const genreOptions = computed(() => {
  return getLibraryMetadataOptions(gamesStore.games, "genres");
});

const platformOptions = computed(() => {
  return getLibraryMetadataOptions(gamesStore.games, "platforms");
});

function clearAdvancedFilters() {
  Object.assign(state, createDefaultLibraryFilterPreset());
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

const hasActiveAdvancedFilters = computed(() => hasActiveLibraryFilters(state));

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
      @next="advanceMetadataQueue"
      @skip-all="clearMetadataQueue"
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
