<script setup lang="ts">
import { Loader2, Search } from "lucide-vue-next";
import type { BackupInfo, Game, SavePathLookup } from "../../../src/types";
import SqobaGameCard from "./SqobaGameCard.vue";
import type { LookupState } from "./types";

const props = defineProps<{
  games: Game[];
  gamesLoading: boolean;
  gamesError: string | null;
  query: string;
  onlyMissing: boolean;
  totalGames: number;
  missingCount: number;
  scanAllLoading: boolean;
  pathsByGameId: Record<string, LookupState<SavePathLookup>>;
  filesByGameId: Record<string, LookupState<BackupInfo | null>>;
  editingGameId: string | null;
  savePathDraft: string;
  savingSavePath: boolean;
}>();

const emit = defineEmits<{
  updateQuery: [value: string];
  updateOnlyMissing: [value: boolean];
  scanAll: [];
  lookupPaths: [game: Game];
  loadFiles: [game: Game];
  beginEdit: [game: Game];
  cancelEdit: [];
  savePathDraftChange: [value: string];
  chooseSaveFolder: [];
  chooseSaveFile: [];
  insertGamePathToken: [];
  openPath: [payload: { game: Game; path: string }];
  saveGamePath: [game: Game];
}>();
</script>

<template>
  <section class="space-y-4">
    <div class="rounded-2xl border border-border/70 bg-card/90 shadow-sm">
      <div class="flex flex-col gap-3 border-b border-border/60 p-4 sm:flex-row sm:items-center">
        <div class="min-w-0">
          <h2 class="text-base font-semibold sm:text-lg">Saves</h2>
          <p class="text-xs text-muted-foreground">
            Games: {{ props.totalGames }} • Missing paths: {{ props.missingCount }}
          </p>
        </div>

        <div class="flex flex-1 flex-col gap-2 sm:flex-row sm:items-center sm:justify-end">
          <div class="relative min-w-0 sm:max-w-xs sm:flex-1">
            <Search class="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <input
              :value="props.query"
              class="h-11 w-full rounded-xl border border-border/70 bg-card/70 pl-9 pr-4 text-sm outline-none"
              placeholder="Search games..."
              @input="emit('updateQuery', ($event.target as HTMLInputElement).value)"
            />
          </div>

          <button
            type="button"
            class="inline-flex items-center justify-center rounded-xl border border-border/70 px-3 py-2 text-sm transition-colors"
            :class="
              props.onlyMissing
                ? 'bg-primary text-primary-foreground hover:bg-accent hover:text-white'
                : 'bg-card/80 hover:bg-accent/70'
            "
            @click="emit('updateOnlyMissing', !props.onlyMissing)"
          >
            Missing only
          </button>

          <button
            type="button"
            class="inline-flex items-center justify-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="props.scanAllLoading || props.games.length === 0"
            @click="emit('scanAll')"
          >
            <Loader2 v-if="props.scanAllLoading" class="h-4 w-4 animate-spin" />
            <Search v-else class="h-4 w-4" />
            Scan visible
          </button>
        </div>
      </div>

      <div v-if="props.gamesLoading" class="flex min-h-64 items-center justify-center p-6">
        <Loader2 class="h-6 w-6 animate-spin" />
      </div>

      <div
        v-else-if="props.gamesError"
        class="m-4 rounded-xl border border-red-500/30 bg-red-500/10 p-4 text-sm"
      >
        <div class="font-medium text-red-200">Games could not be loaded.</div>
        <p class="mt-1 text-red-100/80">{{ props.gamesError }}</p>
      </div>

      <div
        v-else-if="props.games.length === 0 && !props.query && !props.onlyMissing"
        class="m-4 rounded-xl border border-border/70 bg-background/35 p-6 text-center"
      >
        <div class="text-sm font-medium">No games in the library yet.</div>
        <p class="mt-1 text-xs text-muted-foreground">
          Add some games first, then SQOBA can inspect save paths and files.
        </p>
      </div>

      <div
        v-else-if="props.games.length === 0"
        class="m-4 rounded-xl border border-border/70 bg-background/35 p-6 text-center"
      >
        <div class="text-sm font-medium">Nothing matches this filter.</div>
        <p class="mt-1 text-xs text-muted-foreground">
          Try a different search or disable the missing-path filter.
        </p>
      </div>

      <div v-else class="space-y-4 p-4">
        <SqobaGameCard
          v-for="game in props.games"
          :key="game.id"
          :game="game"
          :path-lookup="props.pathsByGameId[game.id]"
          :files-lookup="props.filesByGameId[game.id]"
          :is-editing="props.editingGameId === game.id"
          :save-path-draft="props.savePathDraft"
          :saving-save-path="props.savingSavePath"
          @lookup-paths="emit('lookupPaths', $event)"
          @load-files="emit('loadFiles', $event)"
          @begin-edit="emit('beginEdit', $event)"
          @cancel-edit="emit('cancelEdit')"
          @save-path-draft-change="emit('savePathDraftChange', $event)"
          @choose-save-folder="emit('chooseSaveFolder')"
          @choose-save-file="emit('chooseSaveFile')"
          @insert-game-path-token="emit('insertGamePathToken')"
          @open-path="emit('openPath', $event)"
          @save-game-path="emit('saveGamePath', $event)"
        />
      </div>
    </div>
  </section>
</template>
