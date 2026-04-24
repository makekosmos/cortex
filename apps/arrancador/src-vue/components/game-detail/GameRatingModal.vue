<script setup lang="ts">
import { Save, X } from "lucide-vue-next";

const open = defineModel<boolean>("open", { required: true });
const ratingDraft = defineModel<number>("ratingDraft", { required: true });

const emit = defineEmits<{
  save: [rating: number];
}>();

function saveRating() {
  const rating = Math.max(1, Math.min(7, Number(ratingDraft.value) || 1));
  ratingDraft.value = rating;
  open.value = false;
  emit("save", rating);
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 z-[121] flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm"
      @mousedown.self="open = false"
    >
      <div class="w-full max-w-sm rounded-2xl border border-border/60 bg-card/92 p-6 shadow-[0_30px_80px_rgba(8,12,24,0.55)]">
        <div class="mb-6 flex items-center justify-between">
          <div class="flex h-12 w-12 items-center justify-center rounded-full border border-white/10 bg-white/8 text-2xl font-bold text-white shadow-[0_10px_24px_rgba(0,0,0,0.22)]">
            {{ ratingDraft }}
          </div>
          <button
            type="button"
            class="inline-flex h-10 w-10 items-center justify-center rounded-xl border border-border/70 bg-background/50 transition-colors hover:bg-accent/60"
            @click="open = false"
          >
            <X class="h-4 w-4" />
          </button>
        </div>

        <input
          :value="ratingDraft"
          type="number"
          min="1"
          max="7"
          step="1"
          class="w-full rounded-2xl border border-border/60 bg-background/20 py-6 text-center text-6xl font-bold outline-none"
          @input="ratingDraft = Number(($event.target as HTMLInputElement).value)"
        />

        <div class="mt-6 flex justify-end gap-2">
          <button
            type="button"
            class="inline-flex h-10 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
            @click="open = false"
          >
            Отмена
          </button>
          <button
            type="button"
            class="inline-flex h-10 items-center gap-2 rounded-xl bg-primary px-4 text-sm font-medium text-primary-foreground transition-colors hover:bg-accent hover:text-white"
            @click="saveRating"
          >
            <Save class="h-4 w-4" />
            Сохранить
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
