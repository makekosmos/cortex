<script setup lang="ts">
import { X } from "lucide-vue-next";

defineProps<{
  open: boolean;
  description: string | null;
}>();

const emit = defineEmits<{
  close: [];
}>();
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open && description"
      class="fixed inset-0 z-[120] flex items-center justify-center bg-black/78 p-4 backdrop-blur-sm"
      @mousedown.self="emit('close')"
    >
      <div
        data-testid="game-detail-description-modal"
        class="w-full max-w-2xl overflow-hidden rounded-2xl border border-border/60 bg-card/92 shadow-[0_30px_80px_rgba(8,12,24,0.55)]"
      >
        <div class="flex items-center justify-between border-b border-border/60 px-5 py-4">
          <h2 class="text-lg font-semibold">Описание игры</h2>
          <button
            type="button"
            class="inline-flex h-9 w-9 items-center justify-center rounded-xl border border-border/70 bg-card/80 transition-colors hover:bg-accent/70"
            @click="emit('close')"
          >
            <X class="h-4 w-4" />
          </button>
        </div>
        <div class="max-h-[70vh] overflow-auto px-5 py-4">
          <p class="whitespace-pre-wrap text-sm leading-7 text-foreground/88 sm:text-[15px]">
            {{ description }}
          </p>
        </div>
      </div>
    </div>
  </Teleport>
</template>
