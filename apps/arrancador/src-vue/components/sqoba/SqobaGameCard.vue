<script setup lang="ts">
import {
  ExternalLink,
  File as FileIcon,
  FolderOpen,
  Loader2,
  Pencil,
  Save,
  Search,
  Sparkles,
  X,
} from "lucide-vue-next";
import { computed } from "vue";
import type { BackupInfo, Game, SavePathLookup } from "../../../src/types";
import { GAME_PATH_TOKEN } from "./types";
import type { LookupState } from "./types";

const props = defineProps<{
  game: Game;
  pathLookup?: LookupState<SavePathLookup>;
  filesLookup?: LookupState<BackupInfo | null>;
  isEditing: boolean;
  savePathDraft: string;
  savingSavePath: boolean;
}>();

const emit = defineEmits<{
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

const activeSavePath = computed(() => props.game.save_path ?? props.pathLookup?.data?.save_path ?? null);
const activeSavePathLabel = computed(() => (props.game.save_path ? "Saved path" : "Detected path"));
const candidatePaths = computed(() => props.pathLookup?.data?.candidates ?? []);
const draftPreviewParts = computed(() => props.savePathDraft.split(GAME_PATH_TOKEN));
const fileList = computed(() => props.filesLookup?.data?.files ?? []);
const visibleFiles = computed(() => fileList.value.slice(0, 200));
const hasMoreFiles = computed(() => fileList.value.length > 200);

function formatBytes(bytes: number) {
  if (!bytes || bytes <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = bytes;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  const digits = size >= 10 ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unitIndex]}`;
}
</script>

<template>
  <article class="rounded-2xl border border-border/70 bg-card/90 shadow-sm">
    <div class="border-b border-border/60 p-4">
      <div class="flex flex-wrap items-start gap-3">
        <div class="min-w-0 flex-1">
          <div class="text-sm font-semibold sm:text-base">{{ props.game.name }}</div>
          <div class="mt-1 break-all text-[11px] text-muted-foreground">
            {{ props.game.exe_path }}
          </div>
        </div>

        <div class="flex flex-wrap gap-2">
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-xs transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="props.pathLookup?.loading"
            @click="emit('lookupPaths', props.game)"
          >
            <Loader2 v-if="props.pathLookup?.loading" class="h-3.5 w-3.5 animate-spin" />
            <Search v-else class="h-3.5 w-3.5" />
            Lookup paths
          </button>
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-xs transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="props.filesLookup?.loading"
            @click="emit('loadFiles', props.game)"
          >
            <Loader2 v-if="props.filesLookup?.loading" class="h-3.5 w-3.5 animate-spin" />
            <FileIcon v-else class="h-3.5 w-3.5" />
            List files
          </button>
          <button
            v-if="activeSavePath"
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-xs transition-colors hover:bg-accent/70"
            @click="emit('openPath', { game: props.game, path: activeSavePath })"
          >
            <ExternalLink class="h-3.5 w-3.5" />
            Open
          </button>
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-xs transition-colors hover:bg-accent/70"
            @click="props.isEditing ? emit('cancelEdit') : emit('beginEdit', props.game)"
          >
            <X v-if="props.isEditing" class="h-3.5 w-3.5" />
            <Pencil v-else class="h-3.5 w-3.5" />
            {{ props.isEditing ? "Close" : "Edit path" }}
          </button>
        </div>
      </div>
    </div>

    <div class="space-y-4 p-4">
      <div v-if="activeSavePath" class="rounded-xl border border-emerald-500/25 bg-emerald-500/10 p-3">
        <div class="text-[11px] font-medium uppercase tracking-wide text-emerald-200">
          {{ activeSavePathLabel }}
        </div>
        <div class="mt-1 break-all font-mono text-xs text-foreground">
          {{ activeSavePath }}
        </div>
      </div>

      <div
        v-if="props.pathLookup?.error"
        class="rounded-xl border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-100"
      >
        {{ props.pathLookup.error }}
      </div>

      <div
        v-else-if="props.pathLookup?.data && !props.pathLookup.data.save_path"
        class="rounded-xl border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-amber-100"
      >
        No automatic save path was found. Set one manually below if needed.
      </div>

      <div v-if="candidatePaths.length > 0" class="space-y-2">
        <div class="text-xs font-medium text-muted-foreground">
          Candidate paths
        </div>
        <div class="flex flex-wrap gap-2">
          <button
            v-for="candidate in candidatePaths"
            :key="candidate"
            type="button"
            class="rounded-full border border-border/70 bg-background/50 px-3 py-1 text-left text-[11px] text-muted-foreground transition-colors hover:bg-accent/70 hover:text-foreground"
            @click="emit('openPath', { game: props.game, path: candidate })"
          >
            {{ candidate }}
          </button>
        </div>
      </div>

      <div v-if="props.isEditing" class="rounded-xl border border-border/70 bg-background/45 p-3">
        <div class="text-xs leading-relaxed text-muted-foreground">
          Set a folder or a specific save file. You can prefix the path with
          <span class="mx-1 rounded-md border border-emerald-500/35 bg-emerald-500/10 px-1 py-0.5 font-mono text-[11px] text-emerald-300">
            {{ GAME_PATH_TOKEN }}
          </span>
          to keep it relative to the game executable folder.
        </div>

        <input
          :value="props.savePathDraft"
          class="mt-3 h-11 w-full rounded-xl border border-border/70 bg-card/70 px-4 font-mono text-xs outline-none"
          placeholder="{PATHTOGAME}\\saves or C:\\Users\\...\\Saved Games"
          @input="emit('savePathDraftChange', ($event.target as HTMLInputElement).value)"
        />

        <div
          v-if="props.savePathDraft.includes(GAME_PATH_TOKEN)"
          class="mt-2 text-[11px] leading-relaxed text-muted-foreground"
        >
          Path preview:
          <template v-for="(part, index) in draftPreviewParts" :key="`${index}-${part}`">
            <span>{{ part }}</span>
            <span
              v-if="index < draftPreviewParts.length - 1"
              class="mx-1 rounded-md border border-emerald-500/35 bg-emerald-500/10 px-1 py-0.5 font-mono text-[10px] text-emerald-300"
            >
              {{ GAME_PATH_TOKEN }}
            </span>
          </template>
        </div>

        <div class="mt-3 flex flex-wrap gap-2">
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-xs transition-colors hover:bg-accent/70"
            @click="emit('chooseSaveFolder')"
          >
            <FolderOpen class="h-3.5 w-3.5" />
            Folder
          </button>
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-xs transition-colors hover:bg-accent/70"
            @click="emit('chooseSaveFile')"
          >
            <FileIcon class="h-3.5 w-3.5" />
            File
          </button>
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-xs transition-colors hover:bg-accent/70"
            @click="emit('insertGamePathToken')"
          >
            <Sparkles class="h-3.5 w-3.5" />
            {{ GAME_PATH_TOKEN }}
          </button>
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-xs transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="!props.savePathDraft.trim()"
            @click="emit('openPath', { game: props.game, path: props.savePathDraft.trim() })"
          >
            <ExternalLink class="h-3.5 w-3.5" />
            Open
          </button>

          <div class="flex-1" />

          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl bg-primary px-3 py-2 text-xs font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="props.savingSavePath"
            @click="emit('saveGamePath', props.game)"
          >
            <Loader2 v-if="props.savingSavePath" class="h-3.5 w-3.5 animate-spin" />
            <Save v-else class="h-3.5 w-3.5" />
            Save path
          </button>
        </div>
      </div>

      <div
        v-if="props.filesLookup?.error"
        class="rounded-xl border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-100"
      >
        {{ props.filesLookup.error }}
      </div>

      <div
        v-else-if="props.filesLookup && !props.filesLookup.loading && fileList.length === 0"
        class="rounded-xl border border-border/70 bg-background/45 p-3 text-xs text-muted-foreground"
      >
        No save files were found for the current path.
      </div>

      <div v-else-if="fileList.length > 0" class="rounded-xl border border-border/70 bg-background/45">
        <div class="flex flex-wrap items-center justify-between gap-3 border-b border-border/60 p-3">
          <div>
            <div class="text-sm font-semibold">Save files</div>
            <div class="text-xs text-muted-foreground">
              {{ fileList.length }} files • {{ formatBytes(props.filesLookup?.data?.total_size ?? 0) }}
            </div>
          </div>
          <button
            v-if="props.filesLookup?.data?.save_path"
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-xs transition-colors hover:bg-accent/70"
            @click="emit('openPath', { game: props.game, path: props.filesLookup.data.save_path })"
          >
            <ExternalLink class="h-3.5 w-3.5" />
            Open folder
          </button>
        </div>

        <div class="max-h-56 overflow-auto p-3">
          <div class="space-y-1">
            <div
              v-for="filePath in visibleFiles"
              :key="filePath"
              class="break-all font-mono text-[11px] text-muted-foreground"
            >
              {{ filePath }}
            </div>
          </div>
          <div v-if="hasMoreFiles" class="mt-2 text-xs text-muted-foreground">
            Showing the first 200 files.
          </div>
        </div>
      </div>
    </div>
  </article>
</template>
