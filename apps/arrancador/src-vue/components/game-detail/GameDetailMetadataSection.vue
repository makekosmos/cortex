<script setup lang="ts">
import { Pencil, Search } from "lucide-vue-next";
import type { Game } from "../../../src/types";

defineProps<{
  game: Game;
}>();

const emit = defineEmits<{
  searchRawg: [];
  edit: [];
}>();
</script>

<template>
  <div class="rounded-2xl border border-border/70 bg-card/80 p-4 shadow-sm">
    <div class="mb-4 flex items-center justify-between gap-3">
      <div>
        <div class="text-base font-semibold">Метаданные</div>
        <div class="text-xs text-muted-foreground">
          RAWG, изображения и ручное редактирование
        </div>
      </div>
      <div class="flex gap-2">
        <button
          type="button"
          class="inline-flex h-10 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
          @click="emit('searchRawg')"
        >
          <Search class="h-4 w-4" />
          RAWG
        </button>
        <button
          type="button"
          class="inline-flex h-10 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
          @click="emit('edit')"
        >
          <Pencil class="h-4 w-4" />
          Редактировать
        </button>
      </div>
    </div>

    <div class="grid gap-3 sm:grid-cols-2">
      <div class="rounded-xl border border-border/70 bg-background/40 p-4">
        <div class="text-xs text-muted-foreground">RAWG ID</div>
        <div class="mt-2 text-sm font-medium">{{ game.rawg_id ?? "—" }}</div>
      </div>
      <div class="rounded-xl border border-border/70 bg-background/40 p-4">
        <div class="text-xs text-muted-foreground">Metacritic</div>
        <div class="mt-2 text-sm font-medium">{{ game.metacritic ?? "—" }}</div>
      </div>
      <div class="rounded-xl border border-border/70 bg-background/40 p-4">
        <div class="text-xs text-muted-foreground">Фон</div>
        <div class="mt-2 text-sm font-medium">
          {{ game.background_image ? "Есть" : "Нет" }}
        </div>
      </div>
      <div class="rounded-xl border border-border/70 bg-background/40 p-4">
        <div class="text-xs text-muted-foreground">Обложка</div>
        <div class="mt-2 text-sm font-medium">
          {{ game.cover_image ? "Есть" : "Нет" }}
        </div>
      </div>
    </div>
  </div>
</template>
