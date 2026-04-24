<script setup lang="ts">
import { GamePosterCard } from "@kepler/visuals";
import { Gamepad2 } from "lucide-vue-next";
import { computed } from "vue";
import { RouterLink } from "vue-router";
import { translateGenreListToRu } from "../../src/lib/genres";
import type { Game } from "../../src/types";

const props = defineProps<{
  game: Game;
}>();

const cover = computed(
  () => props.game.background_image || props.game.cover_image,
);
const primaryGenre = computed(
  () => translateGenreListToRu(props.game.genres, 1)[0] ?? null,
);
</script>

<template>
  <GamePosterCard
    :to="`/game/${game.id}`"
    :title="game.name"
    :eyebrow="primaryGenre ?? 'Игра'"
    :cover-src="cover"
    :link-component="RouterLink"
    class-name="min-h-0"
  >
    <template #placeholder>
      <div class="flex h-full w-full items-center justify-center">
        <Gamepad2 class="h-12 w-12 text-muted-foreground" />
      </div>
    </template>
  </GamePosterCard>
</template>
