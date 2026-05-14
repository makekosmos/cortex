<script setup lang="ts">
// LibraryPage — отображение game_obj из ARK.
//
// Адаптация vs `apps/arrancador/src-vue/pages/LibraryPage.vue`:
//   - убраны advanced-filters / LibraryToolbar / drop-zone / RawgMetadataPrompt /
//     Pinia store / install-status composable.
//   - оставлен минимум: search-фильтр + grid карточек.
//   - Запуск игр и автоматический сканер пока работают через legacy
//     Arrancador.exe (Phase 5).

import { computed } from "vue";

import GameCard from "../components/GameCard.vue";
import { useGames } from "../composables/useGames";
import { useSearchQuery } from "../composables/useSearchQuery";

const { games, loading, error } = useGames();
const search = useSearchQuery();

const filteredGames = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return games.value;
  return games.value.filter((g) => g.name.toLowerCase().includes(q));
});
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">Библиотека</h1>

    <p class="arrancador-page__hint">
      Запуск игр и автоматический сканер пока работают через legacy
      Arrancador.exe (Phase 5). Здесь отображаются только game_obj, уже
      синхронизированные в ARK.
    </p>

    <div v-if="error" class="arrancador-error">{{ error }}</div>

    <div v-if="loading && games.length === 0" class="arrancador-empty">
      <p class="arrancador-empty__title">Загрузка…</p>
    </div>

    <div v-else-if="!loading && games.length === 0" class="arrancador-empty">
      <p class="arrancador-empty__title">Библиотека пуста</p>
      <p class="arrancador-empty__hint">
        Добавьте игры через legacy Arrancador.exe — они появятся здесь
        автоматически после синхронизации в ARK.
      </p>
    </div>

    <div v-else-if="filteredGames.length === 0" class="arrancador-empty">
      <p class="arrancador-empty__title">Ничего не найдено</p>
      <p class="arrancador-empty__hint">
        Сбросьте поисковый запрос, чтобы увидеть всю библиотеку.
      </p>
    </div>

    <div v-else class="arrancador-grid">
      <GameCard
        v-for="game in filteredGames"
        :key="game.id"
        :game="game"
      />
    </div>
  </section>
</template>
