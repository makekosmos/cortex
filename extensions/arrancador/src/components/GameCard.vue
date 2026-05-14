<script setup lang="ts">
// GameCard для extension'а.
//
// Адаптация vs `apps/arrancador/src-vue/components/GameCard.vue`:
//   - legacy использует `@kepler/visuals/GamePosterCard` + RouterLink + i18n
//     перевод жанров. Здесь — самостоятельная карточка + router-link на
//     детальную страницу.
//   - Cover берётся из `propsJson.background_image` либо `propsJson.cover_image`
//     (см. `lib/arkGames.ts.projectGame`).

import { computed } from "vue";

import type { ArrancadorGame } from "../lib/arkGames";

const props = defineProps<{
  game: ArrancadorGame;
}>();

const cover = computed(
  () => props.game.backgroundImage ?? props.game.coverImage ?? null,
);
const primaryGenre = computed(() => {
  const genres = props.game.genres;
  if (!genres) return "Игра";
  const first = genres.split(",").map((s) => s.trim()).filter(Boolean)[0];
  return first ?? "Игра";
});
</script>

<template>
  <router-link
    :to="`/game/${game.id}`"
    class="arrancador-card"
    :title="`Открыть страницу игры`"
  >
    <div class="arrancador-card__cover">
      <img v-if="cover" :src="cover" :alt="game.name" />
      <span v-else class="arrancador-card__placeholder" aria-hidden="true">🎮</span>
    </div>
    <div class="arrancador-card__body">
      <span class="arrancador-card__eyebrow">{{ primaryGenre }}</span>
      <span class="arrancador-card__title">{{ game.name }}</span>
    </div>
  </router-link>
</template>
