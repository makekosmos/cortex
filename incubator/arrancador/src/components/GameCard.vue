<script setup lang="ts">
// GameCard — обёртка над `GamePosterCard` из `@kosmos/visuals`.
//
// Адаптация: на route `/game/:id` через RouterLink (linkComponent),
// cover берётся из `propsJson.background_image` либо `propsJson.cover_image`
// (см. `lib/arkGames.ts.projectGame`). Жанр — eyebrow.
import { computed } from "vue";
import { RouterLink } from "vue-router";
import { GamePosterCard } from "@kosmos/visuals";

import type { ArrancadorGame } from "../lib/arkGames";

const props = defineProps<{
  game: ArrancadorGame;
}>();

const cover = computed(() => props.game.backgroundImage ?? props.game.coverImage ?? null);

const primaryGenre = computed(() => {
  const genres = props.game.genres;
  if (!genres) return "Игра";
  const first = genres
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)[0];
  return first ?? "Игра";
});

const to = computed(() => `/game/${props.game.id}`);
</script>

<template>
  <GamePosterCard
    :to="to"
    :title="game.name"
    :eyebrow="primaryGenre"
    :cover-src="cover"
    :link-component="RouterLink"
  >
    <template #placeholder>
      <span class="arrancador-card__placeholder" aria-hidden="true">🎮</span>
    </template>
  </GamePosterCard>
</template>
