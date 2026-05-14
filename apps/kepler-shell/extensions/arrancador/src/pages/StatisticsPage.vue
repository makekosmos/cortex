<script setup lang="ts">
// StatisticsPage — простая агрегация по game_obj из ARK.
//
// Адаптация vs `apps/arrancador/src-vue/pages/StatisticsPage.vue`:
//   - убрана heatmap по дням (требует usage_sessions / events). В extension'е
//     пока агрегация только по game_obj.propsJson.total_playtime.
//   - Тепловая карта по дням / часам — Phase 5+, когда usage queries будут
//     доступны через kepler.ark RPC.

import { computed } from "vue";

import { useGames } from "../composables/useGames";

const { games, loading, error } = useGames();

const totalGames = computed(() => games.value.length);

const totalPlaytime = computed(() =>
  games.value.reduce((sum, g) => sum + (g.totalPlaytime ?? 0), 0),
);

const favouritesCount = computed(
  () => games.value.filter((g) => g.isFavorite).length,
);

const completedCount = computed(
  () => games.value.filter((g) => g.playStatus === "completed").length,
);

const inProgressCount = computed(
  () => games.value.filter((g) => g.playStatus === "in_progress").length,
);

const topPlayed = computed(() =>
  [...games.value]
    .filter((g) => (g.totalPlaytime ?? 0) > 0)
    .sort((a, b) => (b.totalPlaytime ?? 0) - (a.totalPlaytime ?? 0))
    .slice(0, 10),
);

function formatHours(seconds: number): string {
  if (!seconds || seconds <= 0) return "0 ч";
  const hours = seconds / 3600;
  if (hours < 1) {
    const minutes = Math.round(seconds / 60);
    return `${minutes} мин`;
  }
  return `${hours.toFixed(1)} ч`;
}
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">Статистика</h1>

    <p class="arrancador-page__hint">
      Тепловая карта по дням (usage_sessions / events) появится после
      Phase 5. Сейчас агрегация только по `game_obj.propsJson.total_playtime`.
    </p>

    <div v-if="error" class="arrancador-error">{{ error }}</div>

    <div v-if="loading && games.length === 0" class="arrancador-empty">
      <p class="arrancador-empty__title">Загрузка…</p>
    </div>

    <div v-else class="arrancador-stats">
      <div class="arrancador-stats__grid">
        <div class="arrancador-stats__card">
          <span class="arrancador-stats__label">Всего игр</span>
          <span class="arrancador-stats__value">{{ totalGames }}</span>
        </div>
        <div class="arrancador-stats__card">
          <span class="arrancador-stats__label">Общее время</span>
          <span class="arrancador-stats__value">
            {{ formatHours(totalPlaytime) }}
          </span>
        </div>
        <div class="arrancador-stats__card">
          <span class="arrancador-stats__label">В избранном</span>
          <span class="arrancador-stats__value">{{ favouritesCount }}</span>
        </div>
        <div class="arrancador-stats__card">
          <span class="arrancador-stats__label">В процессе</span>
          <span class="arrancador-stats__value">{{ inProgressCount }}</span>
        </div>
        <div class="arrancador-stats__card">
          <span class="arrancador-stats__label">Пройдено</span>
          <span class="arrancador-stats__value">{{ completedCount }}</span>
        </div>
      </div>

      <h2 class="arrancador-stats__subtitle">Топ-10 по времени</h2>

      <div v-if="topPlayed.length === 0" class="arrancador-empty">
        <p class="arrancador-empty__hint">
          Нет данных о наигранных часах. Запустите игру из Arrancador.exe.
        </p>
      </div>
      <ol v-else class="arrancador-stats__top">
        <li
          v-for="game in topPlayed"
          :key="game.id"
          class="arrancador-stats__top-row"
        >
          <span class="arrancador-stats__top-name">{{ game.name }}</span>
          <span class="arrancador-stats__top-value">
            {{ formatHours(game.totalPlaytime ?? 0) }}
          </span>
        </li>
      </ol>
    </div>
  </section>
</template>
