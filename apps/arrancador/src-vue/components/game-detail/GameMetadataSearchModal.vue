<script setup lang="ts">
import { ExternalLink, Gamepad2, Loader2, Search } from "lucide-vue-next";
import type { RawgGame } from "../../../src/types";

const open = defineModel<boolean>("open", { required: true });
const query = defineModel<string>("query", { required: true });
const rename = defineModel<boolean>("rename", { required: true });

defineProps<{
  results: RawgGame[];
  searching: boolean;
  applying: boolean;
}>();

const emit = defineEmits<{
  search: [];
  apply: [result: RawgGame];
}>();
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 z-[123] flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm"
      @mousedown.self="open = false"
    >
      <div class="flex max-h-[80vh] w-full max-w-lg flex-col rounded-lg bg-card">
        <div class="border-b p-4">
          <h2 class="text-lg font-semibold">Поиск метаданных</h2>
          <p class="text-sm text-muted-foreground">
            Поиск информации об игре в базе RAWG
          </p>
        </div>

        <div class="flex flex-1 flex-col overflow-hidden p-4">
          <div class="mb-4 flex gap-2">
            <input
              :value="query"
              placeholder="Название игры..."
              class="h-10 flex-1 rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
              @input="query = ($event.target as HTMLInputElement).value"
              @keydown.enter="emit('search')"
            />
            <button
              type="button"
              class="inline-flex h-10 w-10 items-center justify-center rounded-xl bg-primary text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="searching"
              @click="emit('search')"
            >
              <Loader2 v-if="searching" class="h-4 w-4 animate-spin" />
              <Search v-else class="h-4 w-4" />
            </button>
          </div>

          <label class="mb-3 flex items-center justify-between gap-3 text-sm text-muted-foreground">
            <span>Использовать название из RAWG</span>
            <input
              :checked="rename"
              type="checkbox"
              class="h-4 w-4 rounded border-border/70"
              @change="rename = ($event.target as HTMLInputElement).checked"
            />
          </label>

          <div class="flex-1 overflow-auto">
            <div v-if="results.length > 0" class="space-y-2">
              <button
                v-for="result in results"
                :key="result.id"
                type="button"
                class="flex w-full items-center gap-3 rounded-md border border-transparent p-3 text-left transition-colors hover:border-border hover:bg-secondary/70"
                @click="emit('apply', result)"
              >
                <img
                  v-if="result.background_image"
                  :src="result.background_image"
                  :alt="result.name"
                  class="h-16 w-16 rounded object-cover"
                />
                <div
                  v-else
                  class="flex h-16 w-16 items-center justify-center rounded bg-muted"
                >
                  <Gamepad2 class="h-6 w-6 text-muted-foreground" />
                </div>
                <div class="min-w-0 flex-1">
                  <div class="truncate font-medium">{{ result.name }}</div>
                  <div class="text-sm text-muted-foreground">
                    {{ result.released?.slice(0, 4) || "?" }}
                    <span v-if="result.metacritic"> • {{ result.metacritic }}</span>
                  </div>
                </div>
                <Loader2 v-if="applying" class="h-4 w-4 animate-spin" />
                <ExternalLink v-else class="h-4 w-4 text-muted-foreground" />
              </button>
            </div>
            <div v-else class="py-8 text-center text-muted-foreground">
              Введите название для поиска
            </div>
          </div>
        </div>

        <div class="flex justify-end border-t p-4">
          <button
            type="button"
            class="inline-flex h-10 items-center rounded-xl border border-border/70 bg-card/80 px-4 text-sm transition-colors hover:bg-accent/70"
            @click="open = false"
          >
            Закрыть
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
