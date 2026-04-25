<script setup lang="ts">
import { Filter, Grid3X3, List, SortAsc, X } from "lucide-vue-next";
import type { SortBy, ViewMode } from "../../lib/libraryFilters";

defineProps<{
  totalGames: number;
  favoriteCount: number;
  activeFilterCount: number;
  sortByLabel: Record<SortBy, string>;
}>();

const searchQuery = defineModel<string>("searchQuery", { required: true });
const viewMode = defineModel<ViewMode>("viewMode", { required: true });
const sortBy = defineModel<SortBy>("sortBy", { required: true });
const showAdvancedFilters = defineModel<boolean>("showAdvancedFilters", { required: true });

function readInputValue(event: Event): string {
  return (event.target as HTMLInputElement | null)?.value ?? "";
}

function readSortBy(event: Event): SortBy {
  return ((event.target as HTMLSelectElement | null)?.value ?? "name") as SortBy;
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
      <div>
        <h1 class="text-xl font-bold tracking-tight sm:text-2xl">Библиотека</h1>
        <p class="text-xs text-muted-foreground sm:text-sm">
          {{ totalGames }} игр · {{ favoriteCount }} в избранном
        </p>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          class="inline-flex h-9 items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 text-sm transition-colors hover:bg-accent/70"
          @click="showAdvancedFilters = !showAdvancedFilters"
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
            :class="viewMode === 'grid' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground'"
            @click="viewMode = 'grid'"
          >
            <Grid3X3 class="h-4 w-4" />
          </button>
          <button
            type="button"
            class="inline-flex h-8 w-8 items-center justify-center rounded-lg transition-colors"
            :class="viewMode === 'list' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground'"
            @click="viewMode = 'list'"
          >
            <List class="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>

    <div class="flex flex-col gap-3 xl:flex-row xl:items-center">
      <div class="relative flex-1">
        <input
          :value="searchQuery"
          placeholder="Поиск по библиотеке..."
          class="h-11 w-full rounded-xl border border-border/70 bg-card/70 px-4 pr-10 text-sm outline-none"
          @input="searchQuery = readInputValue($event)"
        />
        <button
          v-if="searchQuery"
          type="button"
          class="absolute top-1/2 right-3 -translate-y-1/2 text-muted-foreground hover:text-foreground"
          @click="searchQuery = ''"
        >
          <X class="h-4 w-4" />
        </button>
      </div>

      <label class="flex items-center gap-2 text-sm text-muted-foreground">
        <SortAsc class="h-4 w-4" />
        <select
          :value="sortBy"
          class="h-11 rounded-xl border border-border/70 bg-card/80 px-3 text-sm text-foreground outline-none"
          @change="sortBy = readSortBy($event)"
        >
          <option v-for="(label, value) in sortByLabel" :key="value" :value="value">
            {{ label }}
          </option>
        </select>
      </label>
    </div>
  </div>
</template>
