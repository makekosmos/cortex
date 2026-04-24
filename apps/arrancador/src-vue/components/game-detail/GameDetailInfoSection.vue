<script setup lang="ts">
import { Pencil } from "lucide-vue-next";
import type { Game } from "../../../src/types";

interface Props {
  game: Game;
}

interface Emits {
  edit: [];
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();
</script>

<template>
  <div class="rounded-2xl border border-border/70 bg-card/80 p-4 shadow-sm">
    <div class="mb-4 flex items-center justify-between gap-3">
      <div>
        <div class="text-base font-semibold">Описание и детали</div>
        <div class="text-xs text-muted-foreground">
          Основные сведения о релизе и локальном пути запуска
        </div>
      </div>
      <button
        type="button"
        class="inline-flex h-10 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
        @click="emit('edit')"
      >
        <Pencil class="h-4 w-4" />
        Редактировать
      </button>
    </div>

    <div class="rounded-xl border border-border/70 bg-background/40 p-4">
      <div class="mb-3 text-xs text-muted-foreground">Описание</div>
      <p class="whitespace-pre-wrap text-sm leading-6 text-foreground/88">
        {{ props.game.description || "Описание отсутствует." }}
      </p>
    </div>

    <div class="mt-4 grid gap-3 sm:grid-cols-2">
      <div class="rounded-xl border border-border/70 bg-background/40 p-4">
        <div class="text-xs text-muted-foreground">Дата выхода</div>
        <div class="mt-2 text-sm font-medium">
          {{ props.game.released ? new Date(props.game.released).toLocaleDateString("ru-RU") : "—" }}
        </div>
      </div>
      <div class="rounded-xl border border-border/70 bg-background/40 p-4">
        <div class="text-xs text-muted-foreground">Платформы</div>
        <div class="mt-2 text-sm font-medium">{{ props.game.platforms || "—" }}</div>
      </div>
      <div class="rounded-xl border border-border/70 bg-background/40 p-4">
        <div class="text-xs text-muted-foreground">Разработчики</div>
        <div class="mt-2 text-sm font-medium">{{ props.game.developers || "—" }}</div>
      </div>
      <div class="rounded-xl border border-border/70 bg-background/40 p-4">
        <div class="text-xs text-muted-foreground">Издатели</div>
        <div class="mt-2 text-sm font-medium">{{ props.game.publishers || "—" }}</div>
      </div>
    </div>

    <div class="mt-4 rounded-xl border border-border/70 bg-background/40 p-4">
      <div class="mb-2 text-xs text-muted-foreground">Исполняемый файл</div>
      <div class="break-all font-mono text-xs text-foreground/88">
        {{ props.game.exe_path }}
      </div>
    </div>
  </div>
</template>
