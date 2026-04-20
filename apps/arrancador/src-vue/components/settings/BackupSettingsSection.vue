<script setup lang="ts">
import { Check, FolderOpen, Loader2, RefreshCw, Shield } from "lucide-vue-next";
import SettingsToggleRow from "./SettingsToggleRow.vue";
import type { InlineFeedback } from "./types";

const props = withDefaults(
  defineProps<{
    sectionId?: string;
    backupDirectory: string;
    maxBackups: number;
    autoBackup: boolean;
    backupBeforeLaunch: boolean;
    manifestRefreshing: boolean;
    manifestFeedback: InlineFeedback | null;
  }>(),
  {
    sectionId: undefined,
  },
);

const emit = defineEmits<{
  chooseBackupDirectory: [];
  refreshManifest: [];
  updateBackupDirectory: [value: string];
  updateMaxBackups: [value: number];
  updateAutoBackup: [value: boolean];
  updateBackupBeforeLaunch: [value: boolean];
}>();

function handleMaxBackupsInput(event: Event) {
  const value = Number.parseInt((event.target as HTMLInputElement).value, 10);
  if (Number.isNaN(value)) {
    return;
  }

  emit("updateMaxBackups", value);
}

function feedbackClass(feedback: InlineFeedback | null) {
  if (!feedback) {
    return "";
  }

  if (feedback.tone === "error") {
    return "text-red-300";
  }

  if (feedback.tone === "success") {
    return "text-emerald-300";
  }

  return "text-muted-foreground";
}
</script>

<template>
  <section :id="props.sectionId" class="space-y-4">
    <div class="flex items-center gap-2">
      <Shield class="h-5 w-5" />
      <h2 class="text-base font-semibold sm:text-lg">Backups</h2>
    </div>

    <div class="space-y-4 rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
      <div>
        <div class="mb-2 flex items-center justify-between gap-3">
          <div class="text-sm font-medium">Backup engine</div>
          <div class="inline-flex items-center gap-1 rounded-full bg-emerald-500/10 px-2 py-0.5 text-xs text-emerald-600 dark:text-emerald-400">
            <Check class="h-3 w-3" />
            Native
          </div>
        </div>
        <p class="text-xs text-muted-foreground">
          Arrancador uses the built-in backup engine with Ludusavi-compatible manifest data.
        </p>
      </div>

      <div class="rounded-xl border border-dashed border-border/70 p-3">
        <div class="flex items-center justify-between gap-3">
          <div>
            <div class="text-sm font-medium">SQOBA manifest</div>
            <div class="text-xs text-muted-foreground">
              Refresh the local save-path manifest used for automatic detection.
            </div>
          </div>
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="props.manifestRefreshing"
            @click="emit('refreshManifest')"
          >
            <Loader2 v-if="props.manifestRefreshing" class="h-3.5 w-3.5 animate-spin" />
            <RefreshCw v-else class="h-3.5 w-3.5" />
            Refresh
          </button>
        </div>
        <div
          v-if="props.manifestFeedback"
          class="mt-2 text-xs"
          :class="feedbackClass(props.manifestFeedback)"
        >
          {{ props.manifestFeedback.text }}
        </div>
      </div>

      <div>
        <label class="mb-2 block text-sm font-medium" for="backup-directory">
          Backup directory
        </label>
        <div class="flex gap-2">
          <input
            id="backup-directory"
            :value="props.backupDirectory"
            class="flex h-11 flex-1 rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
            placeholder="Choose a folder for backups"
            @input="emit('updateBackupDirectory', ($event.target as HTMLInputElement).value)"
          />
          <button
            type="button"
            class="inline-flex h-11 w-11 items-center justify-center rounded-xl border border-border/70 bg-card/80 transition-colors hover:bg-accent/70"
            aria-label="Choose backup directory"
            @click="emit('chooseBackupDirectory')"
          >
            <FolderOpen class="h-4 w-4" />
          </button>
        </div>
      </div>

      <div>
        <label class="mb-2 block text-sm font-medium" for="max-backups">
          Max backups per game
        </label>
        <input
          id="max-backups"
          type="number"
          min="1"
          max="100"
          :value="props.maxBackups"
          class="h-11 w-28 rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
          @input="handleMaxBackupsInput"
        />
        <p class="mt-1 text-xs text-muted-foreground">
          Older backups are pruned after the limit is exceeded.
        </p>
      </div>

      <div class="space-y-2">
        <SettingsToggleRow
          id="setting-auto-backup"
          :model-value="props.autoBackup"
          label="Enable automatic backups"
          @update:model-value="emit('updateAutoBackup', $event)"
        />
        <SettingsToggleRow
          id="setting-backup-before-launch"
          :model-value="props.backupBeforeLaunch"
          label="Suggest a backup before launch"
          @update:model-value="emit('updateBackupBeforeLaunch', $event)"
        />
      </div>
    </div>
  </section>
</template>
