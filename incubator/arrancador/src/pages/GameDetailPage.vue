<script setup lang="ts">
// GameDetailPage — read-only детальная страница game_obj.
//
// Адаптация vs `apps/arrancador/src-vue/pages/GameDetailPage.vue`:
//   - убраны launching / backup management / rating editor / process bindings /
//     RAWG metadata search. Всё это требует write paths и main-process IPC.
//   - оставлен hero (картинка + название + жанры + описание) и read-only
//     метаданные. Действия (запуск, бэкап, привязки) — Phase 5+.

import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";

import { EmptyState } from "@kosmos/visuals";

import { useGames } from "../composables/useGames";

const route = useRoute();
const router = useRouter();
const { games, loading, error } = useGames();

const gameId = computed(() => (typeof route.params.id === "string" ? route.params.id : ""));

const game = computed(() => games.value.find((g) => g.id === gameId.value) ?? null);

const heroImage = computed(() => game.value?.backgroundImage ?? game.value?.coverImage ?? null);

const genres = computed(() => {
  const raw = game.value?.genres;
  if (!raw) return null;
  return raw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
    .slice(0, 3)
    .join(" · ");
});

const releaseYear = computed(() => {
  const released = game.value?.released;
  if (!released) return null;
  const year = new Date(released).getFullYear();
  return Number.isFinite(year) ? String(year) : null;
});

const playtimeLabel = computed(() => {
  const seconds = game.value?.totalPlaytime ?? 0;
  if (seconds <= 0) return "Не запускалась";
  const hours = seconds / 3600;
  return hours < 1 ? `${Math.round(seconds / 60)} мин` : `${hours.toFixed(1)} ч`;
});

function goBack() {
  router.back();
}
</script>

<template>
  <section class="arrancador-page">
    <button type="button" class="arrancador-back" @click="goBack">← Назад</button>

    <div v-if="error" class="arrancador-error">{{ error }}</div>

    <EmptyState v-if="loading && games.length === 0" title="Загрузка…" />

    <EmptyState
      v-else-if="!game"
      title="Игра не найдена"
      description="Возможно, она была удалена из ARK. Вернитесь в Библиотеку."
    />

    <article v-else class="arrancador-detail">
      <div
        class="arrancador-detail__hero"
        :style="heroImage ? { backgroundImage: `url(${heroImage})` } : undefined"
      >
        <div class="arrancador-detail__hero-overlay">
          <h1 class="arrancador-detail__title">{{ game.name }}</h1>
          <div v-if="genres" class="arrancador-detail__genres">
            {{ genres }}
          </div>
        </div>
      </div>

      <div class="arrancador-detail__meta">
        <div v-if="releaseYear" class="arrancador-detail__meta-row">
          <span class="arrancador-detail__meta-label">Год выпуска</span>
          <span class="arrancador-detail__meta-value">{{ releaseYear }}</span>
        </div>
        <div class="arrancador-detail__meta-row">
          <span class="arrancador-detail__meta-label">Время</span>
          <span class="arrancador-detail__meta-value">{{ playtimeLabel }}</span>
        </div>
        <div v-if="game.userRating" class="arrancador-detail__meta-row">
          <span class="arrancador-detail__meta-label">Оценка</span>
          <span class="arrancador-detail__meta-value"> {{ game.userRating.toFixed(1) }} / 5 </span>
        </div>
        <div v-if="game.playStatus" class="arrancador-detail__meta-row">
          <span class="arrancador-detail__meta-label">Статус</span>
          <span class="arrancador-detail__meta-value">
            {{ game.playStatus }}
          </span>
        </div>
        <div v-if="game.exePath" class="arrancador-detail__meta-row">
          <span class="arrancador-detail__meta-label">Путь</span>
          <span class="arrancador-detail__meta-value arrancador-detail__path">
            {{ game.exePath }}
          </span>
        </div>
      </div>

      <p v-if="game.description" class="arrancador-detail__description">
        {{ game.description }}
      </p>

      <p v-if="game.userNote" class="arrancador-detail__note">
        <strong>Заметка:</strong> {{ game.userNote }}
      </p>

      <p class="arrancador-page__hint">
        Запуск, бэкапы и редактирование метаданных пока в legacy Arrancador.exe (Phase 5).
      </p>
    </article>
  </section>
</template>
