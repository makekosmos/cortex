<script setup lang="ts">
import { Loader2, Save, Star, Timer } from "lucide-vue-next";
import type { Game } from "../../../src/types";
import {
  formatPlaytime,
  PLAY_STATUS_LABELS,
  PLAY_STATUS_TONES,
} from "../../lib/gameDetailDisplay";

interface Props {
  totalPlaytime: number;
  savingRating: boolean;
  savingNote: boolean;
  playStatus: Game["play_status"];
  userRating: number | null;
  userNote: string;
}

interface Emits {
  "update:playStatus": [value: Game["play_status"]];
  "update:userRating": [value: number | null];
  "update:userNote": [value: string];
  savePlayStatus: [];
  saveUserRating: [];
  openRatingModal: [];
  saveUserNote: [];
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

function updatePlayStatus(value: string) {
  if (
    value === "not_started" ||
    value === "in_progress" ||
    value === "completed" ||
    value === "abandoned"
  ) {
    emit("update:playStatus", value);
  }
}

function updateUserRating(value: string) {
  const next = value.trim() === "" ? null : Number(value);
  emit("update:userRating", next !== null && Number.isFinite(next) ? next : null);
}
</script>

<template>
  <div class="rounded-2xl border border-border/70 bg-card/80 p-4 shadow-sm">
    <div class="mb-4 flex items-center justify-between gap-3">
      <div>
        <div class="text-base font-semibold">Состояние игры</div>
        <div class="text-xs text-muted-foreground">
          Запуск, статус прохождения и личные данные
        </div>
      </div>
      <select
        :value="props.playStatus"
        class="h-10 rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
        @change="
          updatePlayStatus(($event.target as HTMLSelectElement).value);
          emit('savePlayStatus');
        "
      >
        <option value="not_started">Не начато</option>
        <option value="in_progress">В процессе</option>
        <option value="completed">Пройдено</option>
        <option value="abandoned">Брошено</option>
      </select>
    </div>

    <div class="grid gap-4 sm:grid-cols-2">
      <div class="rounded-xl border border-border/70 bg-background/40 p-4">
        <div class="text-xs text-muted-foreground">Личный рейтинг</div>
        <div class="mt-3 flex items-center gap-3">
          <input
            :value="props.userRating ?? ''"
            type="number"
            min="1"
            max="7"
            class="h-11 w-24 rounded-xl border border-border/70 bg-card/70 px-3 text-lg font-semibold outline-none"
            @input="updateUserRating(($event.target as HTMLInputElement).value)"
          />
          <button
            type="button"
            class="inline-flex h-11 items-center gap-2 rounded-xl bg-primary px-4 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="props.savingRating"
            @click="emit('saveUserRating')"
          >
            <Loader2 v-if="props.savingRating" class="h-4 w-4 animate-spin" />
            <Save v-else class="h-4 w-4" />
            Сохранить
          </button>
          <button
            type="button"
            class="inline-flex h-11 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
            @click="emit('openRatingModal')"
          >
            <Star class="h-4 w-4" />
            Быстрый ввод
          </button>
        </div>
      </div>

      <div class="rounded-xl border border-border/70 bg-background/40 p-4">
        <div class="text-xs text-muted-foreground">Время в игре</div>
        <div class="mt-3 flex items-center gap-2 text-2xl font-semibold">
          <Timer class="h-5 w-5" />
          {{ formatPlaytime(props.totalPlaytime) }}
        </div>
        <div class="mt-3">
          <span class="rounded-full border px-2 py-1 text-xs" :class="PLAY_STATUS_TONES[props.playStatus]">
            {{ PLAY_STATUS_LABELS[props.playStatus] }}
          </span>
        </div>
      </div>
    </div>

    <div class="mt-4 rounded-xl border border-border/70 bg-background/40 p-4">
      <div class="mb-2 text-xs text-muted-foreground">Личная заметка</div>
      <textarea
        :value="props.userNote"
        class="min-h-[160px] w-full rounded-xl border border-border/70 bg-card/70 px-4 py-3 text-sm outline-none"
        @input="emit('update:userNote', ($event.target as HTMLTextAreaElement).value)"
        placeholder="Что стоит помнить об этой игре?"
      />
      <div class="mt-3 flex justify-end">
        <button
          type="button"
          class="inline-flex h-10 items-center gap-2 rounded-xl bg-primary px-4 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="props.savingNote"
          @click="emit('saveUserNote')"
        >
          <Loader2 v-if="props.savingNote" class="h-4 w-4 animate-spin" />
          <Save v-else class="h-4 w-4" />
          Сохранить заметку
        </button>
      </div>
    </div>
  </div>
</template>
