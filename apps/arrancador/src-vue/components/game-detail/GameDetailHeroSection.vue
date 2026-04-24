<script setup lang="ts">
import { Activity, HardDrive, Heart, Loader2, Play } from "lucide-vue-next";
import type { Game } from "../../../src/types";

interface Props {
  game: Game;
  heroImage: string | null;
  heroGenres: string | null;
  heroDescription: string | null;
  heroMeta: string;
  launching: boolean;
  restoring: boolean;
  isMissing: boolean;
  runningCount: number;
  checkingRunning: boolean;
}

interface Emits {
  launch: [];
  toggleFavorite: [];
  showDescription: [];
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();
</script>

<template>
  <section
    data-testid="game-detail-hero"
    class="overflow-hidden rounded-[28px] border border-border/70 bg-card/80 shadow-[0_24px_60px_rgba(0,0,0,0.24)]"
  >
    <div class="relative min-h-[360px] overflow-hidden">
      <img
        v-if="props.heroImage"
        :src="props.heroImage"
        :alt="props.game.name"
        class="absolute inset-0 h-full w-full object-cover"
      />
      <div
        v-else
        class="absolute inset-0 bg-[radial-gradient(circle_at_top_right,rgba(129,140,248,0.4),transparent_45%),linear-gradient(135deg,rgba(23,23,28,0.96),rgba(55,65,81,0.92))]"
      />
      <div class="absolute inset-0 bg-gradient-to-t from-black via-black/50 to-black/10" />
      <div class="relative flex min-h-[360px] flex-col justify-end p-6 sm:p-8">
        <div class="max-w-[480px] space-y-3">
          <h1 class="text-3xl font-semibold tracking-tight text-white sm:text-4xl">
            {{ props.game.name }}
          </h1>
          <div v-if="props.heroGenres" class="text-xs uppercase tracking-[0.22em] text-white/70">
            {{ props.heroGenres }}
          </div>
          <p
            v-if="props.heroDescription"
            data-testid="game-detail-hero-description"
            class="max-w-[440px] overflow-hidden text-sm leading-6 text-white/78 [display:-webkit-box] [-webkit-box-orient:vertical] [-webkit-line-clamp:2]"
          >
            {{ props.heroDescription }}
          </p>
          <div class="text-sm text-white/70">
            {{ props.heroMeta }}
          </div>
        </div>

        <div class="mt-6 flex flex-wrap items-end gap-3">
          <button
            type="button"
            class="inline-flex h-11 items-center gap-2 rounded-full bg-primary px-5 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="props.launching || props.restoring || props.isMissing"
            @click="emit('launch')"
          >
            <HardDrive v-if="props.isMissing" class="h-4 w-4" />
            <Activity v-else-if="props.runningCount > 0" class="h-4 w-4" />
            <Loader2
              v-else-if="props.launching || props.restoring || props.checkingRunning"
              class="h-4 w-4 animate-spin"
            />
            <Play v-else class="h-4 w-4" />
            {{
              props.isMissing
                ? "Не установлена"
                : props.runningCount > 0
                  ? "Закрыть игру"
                  : "Играть"
            }}
          </button>

          <button
            type="button"
            class="inline-flex h-11 items-center gap-2 rounded-full border border-white/15 bg-black/20 px-4 text-sm text-white transition-colors hover:bg-black/35"
            @click="emit('toggleFavorite')"
          >
            <Heart class="h-4 w-4" :class="{ 'fill-current text-rose-400': props.game.is_favorite }" />
            {{ props.game.is_favorite ? "В избранном" : "В избранное" }}
          </button>

          <button
            type="button"
            data-testid="game-detail-description-button"
            class="inline-flex h-11 items-center gap-2 rounded-full border border-white/15 bg-black/20 px-4 text-sm text-white transition-colors hover:bg-black/35"
            @click="emit('showDescription')"
          >
            Описание
          </button>
        </div>
      </div>
    </div>
  </section>
</template>
