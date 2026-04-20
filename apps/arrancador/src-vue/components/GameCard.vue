<script setup lang="ts">
import { computed } from "vue";
import { Gamepad2 } from "lucide-vue-next";
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
  <RouterLink
    :to="`/game/${game.id}`"
    class="group block overflow-hidden rounded-[22px] border border-border/70 bg-card/80 shadow-[0_18px_40px_rgba(0,0,0,0.22)] transition-colors hover:border-border"
  >
    <div class="relative aspect-[0.76] overflow-hidden bg-muted">
      <img
        v-if="cover"
        :src="cover"
        :alt="game.name"
        class="h-full w-full object-cover"
        loading="lazy"
        decoding="async"
      />
      <div v-else class="flex h-full w-full items-center justify-center">
        <Gamepad2 class="h-12 w-12 text-muted-foreground" />
      </div>

      <div class="game-card-overlay absolute inset-x-0 bottom-0 p-4">
        <div class="text-[11px] uppercase tracking-[0.18em] text-white/70">
          {{ primaryGenre ?? "Игра" }}
        </div>
        <div class="mt-2 line-clamp-2 text-base font-semibold text-white">
          {{ game.name }}
        </div>
      </div>
    </div>
  </RouterLink>
</template>
