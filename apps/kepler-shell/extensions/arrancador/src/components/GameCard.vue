<script setup lang="ts">
// GameCard для extension'а.
//
// Адаптация vs `apps/arrancador/src-vue/components/GameCard.vue`:
//   - legacy использует `@kosmos/visuals/GamePosterCard` + RouterLink + i18n
//     перевод жанров. Здесь — самостоятельная карточка без router'а.
//   - Launch игры (по клику) намеренно не реализован: для spawn-процесса нужен
//     main-process IPC, которого extension не имеет. Tooltip объясняет, что
//     запуск пока через legacy Arrancador.exe (Phase 5).
//   - Cover берётся из `propsJson.background_image` либо `propsJson.cover_image`
//     (см. `ark-game-objects.ts.mergeArkGameProps`).

import type { ArrancadorGame } from "../pages/LayoutPage.vue";
import { computed } from "vue";

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
  <article
    class="arrancador-card"
    :title="`Запуск через legacy Arrancador.exe (Phase 5)`"
  >
    <div class="arrancador-card__cover">
      <img v-if="cover" :src="cover" :alt="game.name" />
      <span v-else class="arrancador-card__placeholder" aria-hidden="true">🎮</span>
    </div>
    <div class="arrancador-card__body">
      <span class="arrancador-card__eyebrow">{{ primaryGenre }}</span>
      <span class="arrancador-card__title">{{ game.name }}</span>
    </div>
  </article>
</template>
