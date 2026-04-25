<script setup lang="ts">
import { Trash2 } from "lucide-vue-next";

const emit = defineEmits<{
  delete: [];
}>();
</script>

<template>
  <div class="rounded-2xl border border-border/70 bg-card/80 p-4 shadow-sm">
    <div class="mb-4 flex items-center justify-between gap-3">
      <div>
        <div class="text-base font-semibold">Опасные действия</div>
        <div class="text-xs text-muted-foreground">Удаление записи из библиотеки</div>
      </div>
    </div>

    <button
      type="button"
      class="inline-flex items-center gap-2 rounded-xl border border-red-500/35 bg-red-500/10 px-4 py-2 text-sm text-red-300 transition-colors hover:bg-red-500/15"
      @click="emit('delete')"
    >
      <Trash2 class="h-4 w-4" />
      Удалить игру
    </button>
  </div>
</template>
