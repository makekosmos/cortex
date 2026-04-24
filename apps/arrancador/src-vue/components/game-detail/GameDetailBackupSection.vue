<script setup lang="ts">
import {
  ExternalLink,
  File as FileIcon,
  FolderOpen,
  Loader2,
  Save,
  Search,
  Shield,
  Star,
} from "lucide-vue-next";
import type { Backup } from "../../../src/types";
import { formatBytes, GAME_PATH_TOKEN } from "../../lib/gameDetailDisplay";

const savePathDraft = defineModel<string>("savePathDraft", { required: true });
const showAllBackups = defineModel<boolean>("showAllBackups", { required: true });

defineProps<{
  backupEnabled: boolean;
  creatingBackup: boolean;
  savingPath: boolean;
  locatingSavePath: boolean;
  canOpenSavePath: boolean;
  savePathPreviewParts: string[];
  backups: Backup[];
  latestBackup: Backup | null;
  olderBackups: Backup[];
  loadingBackups: boolean;
  restoring: boolean;
}>();

const emit = defineEmits<{
  toggleBackupEnabled: [enabled: boolean];
  createBackup: [];
  chooseSaveFolder: [];
  chooseSaveFile: [];
  insertGamePathToken: [];
  locateSavePath: [];
  openSavePath: [];
  saveGamePath: [];
  restoreBackup: [backupId: string];
}>();

function onToggleBackup(event: Event) {
  emit("toggleBackupEnabled", (event.target as HTMLInputElement).checked);
}
</script>

<template>
  <div class="rounded-2xl border border-border/70 bg-card/80 p-4 shadow-sm">
    <div class="mb-4 flex items-center justify-between gap-3">
      <div>
        <div class="text-base font-semibold">Сохранения и бэкапы</div>
        <div class="text-xs text-muted-foreground">
          Путь к сейвам и история бэкапов
        </div>
      </div>
      <div class="flex items-center gap-3">
        <label class="flex items-center gap-2 text-xs text-muted-foreground">
          <input
            :checked="backupEnabled"
            type="checkbox"
            class="h-4 w-4 rounded border-border/70"
            @change="onToggleBackup"
          />
          Автобэкап
        </label>
        <button
          type="button"
          class="inline-flex h-10 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60 disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="creatingBackup"
          @click="emit('createBackup')"
        >
          <Loader2 v-if="creatingBackup" class="h-4 w-4 animate-spin" />
          <Shield v-else class="h-4 w-4" />
          Создать бэкап
        </button>
      </div>
    </div>

    <div class="rounded-xl border border-border/70 bg-background/40 p-4">
      <div class="mb-2 text-xs text-muted-foreground">Путь сохранений</div>
      <input
        :value="savePathDraft"
        class="h-11 w-full rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
        placeholder="Например: {PATHTOGAME}\\saves"
        @input="savePathDraft = ($event.target as HTMLInputElement).value"
      />

      <div
        v-if="savePathDraft.includes(GAME_PATH_TOKEN)"
        class="mt-3 text-[11px] text-muted-foreground"
      >
        Путь:
        <template v-for="(part, index) in savePathPreviewParts" :key="`${index}-${part}`">
          <span>{{ part }}</span>
          <span
            v-if="index < savePathPreviewParts.length - 1"
            class="mx-1 rounded-md border border-emerald-500/35 bg-emerald-500/10 px-1 py-0.5 font-mono text-[10px] text-emerald-300"
          >
            {{ GAME_PATH_TOKEN }}
          </span>
        </template>
      </div>

      <div class="mt-3 flex flex-wrap gap-2">
        <button
          type="button"
          class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70"
          @click="emit('chooseSaveFolder')"
        >
          <FolderOpen class="h-4 w-4" />
          Папка
        </button>
        <button
          type="button"
          class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70"
          @click="emit('chooseSaveFile')"
        >
          <FileIcon class="h-4 w-4" />
          Файл
        </button>
        <button
          type="button"
          class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70"
          @click="emit('insertGamePathToken')"
        >
          <Star class="h-4 w-4" />
          {{ GAME_PATH_TOKEN }}
        </button>
        <button
          type="button"
          class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="locatingSavePath"
          @click="emit('locateSavePath')"
        >
          <Loader2 v-if="locatingSavePath" class="h-4 w-4 animate-spin" />
          <Search v-else class="h-4 w-4" />
          Найти
        </button>
        <button
          type="button"
          class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="!canOpenSavePath"
          @click="emit('openSavePath')"
        >
          <ExternalLink class="h-4 w-4" />
          Открыть
        </button>
        <button
          type="button"
          class="ml-auto inline-flex items-center gap-2 rounded-xl bg-primary px-4 py-2 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="savingPath"
          @click="emit('saveGamePath')"
        >
          <Loader2 v-if="savingPath" class="h-4 w-4 animate-spin" />
          <Save v-else class="h-4 w-4" />
          Сохранить путь
        </button>
      </div>
    </div>

    <div class="mt-4 rounded-xl border border-border/70 bg-background/40">
      <div class="flex items-center justify-between border-b border-border/60 px-4 py-3">
        <div>
          <div class="text-sm font-medium">Последние бэкапы</div>
          <div class="text-xs text-muted-foreground">
            {{ backups.length }} записей
          </div>
        </div>
      </div>

      <div v-if="loadingBackups" class="flex items-center gap-2 px-4 py-4 text-sm text-muted-foreground">
        <Loader2 class="h-4 w-4 animate-spin" />
        Загрузка бэкапов...
      </div>

      <div v-else-if="backups.length === 0" class="px-4 py-4 text-sm text-muted-foreground">
        Бэкапов пока нет.
      </div>

      <div v-else class="space-y-2 p-4">
        <div
          v-if="latestBackup"
          class="rounded-xl border border-border/70 bg-card/70 p-4"
        >
          <div class="flex items-start justify-between gap-3">
            <div>
              <div class="text-sm font-medium">Последний бэкап</div>
              <div class="mt-1 text-xs text-muted-foreground">
                {{ new Date(latestBackup.created_at).toLocaleString("ru-RU") }} •
                {{ formatBytes(latestBackup.backup_size) }}
              </div>
            </div>
            <button
              type="button"
              class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-3 py-2 text-sm transition-colors hover:bg-accent/60 disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="restoring"
              @click="emit('restoreBackup', latestBackup.id)"
            >
              <Loader2 v-if="restoring" class="h-4 w-4 animate-spin" />
              <Shield v-else class="h-4 w-4" />
              Восстановить
            </button>
          </div>
        </div>

        <div v-if="olderBackups.length > 0">
          <button
            type="button"
            class="text-xs text-muted-foreground transition-colors hover:text-foreground"
            @click="showAllBackups = !showAllBackups"
          >
            {{ showAllBackups ? "Скрыть историю" : `Показать историю (${olderBackups.length})` }}
          </button>
          <div v-if="showAllBackups" class="mt-2 space-y-2">
            <div
              v-for="backup in olderBackups"
              :key="backup.id"
              class="flex items-center justify-between gap-3 rounded-xl border border-border/70 bg-card/60 px-4 py-3"
            >
              <div>
                <div class="text-sm font-medium">
                  {{ new Date(backup.created_at).toLocaleString("ru-RU") }}
                </div>
                <div class="text-xs text-muted-foreground">
                  {{ formatBytes(backup.backup_size) }}
                  <span v-if="backup.is_auto"> • авто</span>
                </div>
              </div>
              <button
                type="button"
                class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-3 py-2 text-sm transition-colors hover:bg-accent/60"
                @click="emit('restoreBackup', backup.id)"
              >
                <Shield class="h-4 w-4" />
                Восстановить
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
