<script setup lang="ts">
import { ListFilter, Star, X } from "lucide-vue-next";
import type {
  InstallState,
  PlayedState,
  PlayStatusState,
  RatingMode,
} from "../../lib/libraryFilters";

defineProps<{
  genreOptions: readonly string[];
  platformOptions: readonly string[];
  playedStateLabel: Record<PlayedState, string>;
  installStateLabel: Record<InstallState, string>;
  playStatusLabel: Record<PlayStatusState, string>;
  translateGenre: (genre: string) => string;
}>();

const emit = defineEmits<{
  clearFilters: [];
}>();

const showFavoritesOnly = defineModel<boolean>("showFavoritesOnly", { required: true });
const selectedGenres = defineModel<string[]>("selectedGenres", { required: true });
const selectedPlatforms = defineModel<string[]>("selectedPlatforms", { required: true });
const playedState = defineModel<PlayedState>("playedState", { required: true });
const installState = defineModel<InstallState>("installState", { required: true });
const playStatusState = defineModel<PlayStatusState>("playStatusState", { required: true });
const ratingMode = defineModel<RatingMode>("ratingMode", { required: true });
const minRating = defineModel<string>("minRating", { required: true });
const maxRating = defineModel<string>("maxRating", { required: true });
const minMetacritic = defineModel<string>("minMetacritic", { required: true });
const maxMetacritic = defineModel<string>("maxMetacritic", { required: true });
const minPlaytimeHours = defineModel<string>("minPlaytimeHours", { required: true });
const maxPlaytimeHours = defineModel<string>("maxPlaytimeHours", { required: true });
const requireMetadata = defineModel<boolean>("requireMetadata", { required: true });

function readSelectedOptions(event: Event): string[] {
  const target = event.target as HTMLSelectElement | null;
  if (!target) return [];
  return Array.from(target.selectedOptions).map((option) => option.value);
}

function readInputValue(event: Event): string {
  return (event.target as HTMLInputElement | null)?.value ?? "";
}

function readPlayedState(event: Event): PlayedState {
  return ((event.target as HTMLSelectElement | null)?.value ?? "all") as PlayedState;
}

function readInstallState(event: Event): InstallState {
  return ((event.target as HTMLSelectElement | null)?.value ?? "all") as InstallState;
}

function readPlayStatusState(event: Event): PlayStatusState {
  return ((event.target as HTMLSelectElement | null)?.value ?? "all") as PlayStatusState;
}

function readRatingMode(event: Event): RatingMode {
  return ((event.target as HTMLSelectElement | null)?.value ?? "user") as RatingMode;
}
</script>

<template>
  <div class="rounded-xl border border-border/70 bg-card/40 p-4">
    <div class="mb-4 flex flex-wrap items-center gap-2">
      <button
        type="button"
        class="inline-flex items-center gap-2 rounded-xl border px-3 py-2 text-sm transition-colors"
        :class="
          showFavoritesOnly
            ? 'border-primary bg-primary text-primary-foreground'
            : 'border-border/70 bg-background/50 hover:bg-accent/60'
        "
        @click="showFavoritesOnly = !showFavoritesOnly"
      >
        <Star class="h-4 w-4" :class="{ 'fill-current': showFavoritesOnly }" />
        Избранное
      </button>

      <button
        type="button"
        class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-3 py-2 text-sm transition-colors hover:bg-accent/60"
        @click="emit('clearFilters')"
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
          :value="selectedGenres"
          @change="selectedGenres = readSelectedOptions($event)"
        >
          <option v-for="genre in genreOptions" :key="genre" :value="genre">
            {{ translateGenre(genre) }}
          </option>
        </select>
      </label>

      <label class="space-y-1 text-sm">
        <div class="text-xs text-muted-foreground">Платформы</div>
        <select
          multiple
          class="min-h-28 w-full rounded-xl border border-border/70 bg-background/50 px-3 py-2 text-sm outline-none"
          :value="selectedPlatforms"
          @change="selectedPlatforms = readSelectedOptions($event)"
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
            :value="playedState"
            class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
            @change="playedState = readPlayedState($event)"
          >
            <option v-for="(label, value) in playedStateLabel" :key="value" :value="value">
              {{ label }}
            </option>
          </select>
        </label>

        <label class="space-y-1 text-sm">
          <div class="text-xs text-muted-foreground">Установка</div>
          <select
            :value="installState"
            class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
            @change="installState = readInstallState($event)"
          >
            <option v-for="(label, value) in installStateLabel" :key="value" :value="value">
              {{ label }}
            </option>
          </select>
        </label>

        <label class="space-y-1 text-sm">
          <div class="text-xs text-muted-foreground">Прохождение</div>
          <select
            :value="playStatusState"
            class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
            @change="playStatusState = readPlayStatusState($event)"
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
              :value="minRating"
              inputmode="decimal"
              placeholder="Мин."
              class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
              @input="minRating = readInputValue($event)"
            />
            <input
              :value="maxRating"
              inputmode="decimal"
              placeholder="Макс."
              class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
              @input="maxRating = readInputValue($event)"
            />
          </div>
        </label>

        <label class="space-y-1 text-sm">
          <div class="text-xs text-muted-foreground">Источник рейтинга</div>
          <select
            :value="ratingMode"
            class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
            @change="ratingMode = readRatingMode($event)"
          >
            <option value="user">Личный рейтинг</option>
            <option value="metacritic">Metacritic</option>
          </select>
        </label>

        <label class="space-y-1 text-sm">
          <div class="text-xs text-muted-foreground">Metacritic</div>
          <div class="flex gap-2">
            <input
              :value="minMetacritic"
              inputmode="decimal"
              placeholder="Мин."
              class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
              @input="minMetacritic = readInputValue($event)"
            />
            <input
              :value="maxMetacritic"
              inputmode="decimal"
              placeholder="Макс."
              class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
              @input="maxMetacritic = readInputValue($event)"
            />
          </div>
        </label>

        <label class="space-y-1 text-sm">
          <div class="text-xs text-muted-foreground">Время в игре (часы)</div>
          <div class="flex gap-2">
            <input
              :value="minPlaytimeHours"
              inputmode="decimal"
              placeholder="Мин."
              class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
              @input="minPlaytimeHours = readInputValue($event)"
            />
            <input
              :value="maxPlaytimeHours"
              inputmode="decimal"
              placeholder="Макс."
              class="h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
              @input="maxPlaytimeHours = readInputValue($event)"
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
          requireMetadata
            ? 'border-primary bg-primary text-primary-foreground'
            : 'border-border/70 bg-background/50 hover:bg-accent/60'
        "
        @click="requireMetadata = !requireMetadata"
      >
        <ListFilter class="h-4 w-4" />
        {{ requireMetadata ? "Только с метаданными" : "Нужны метаданные" }}
      </button>
    </div>
  </div>
</template>
