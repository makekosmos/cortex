<script setup lang="ts">
import { Clock, Gamepad2, Play, Star } from "lucide-vue-next";
import { RouterLink } from "vue-router";
import { translateGenreListToRu } from "@/lib/genres";
import type { Game } from "@/types";
import { formatPlaytime, type ViewMode } from "../../lib/libraryFilters";
import GameCard from "../GameCard.vue";

defineProps<{
  games: readonly Game[];
  totalGameCount: number;
  viewMode: ViewMode;
}>();
</script>

<template>
  <div
    v-if="games.length === 0"
    class="flex flex-1 flex-col items-center justify-center py-16 text-center"
  >
    <Gamepad2 class="mb-4 h-12 w-12 text-muted-foreground" />
    <template v-if="totalGameCount === 0">
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

  <div
    v-else-if="viewMode === 'grid'"
    class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4"
  >
    <GameCard v-for="game in games" :key="game.id" :game="game" />
  </div>

  <div v-else class="space-y-2">
    <RouterLink
      v-for="game in games"
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
</template>
