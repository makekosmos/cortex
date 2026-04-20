<script setup lang="ts">
import { Check, FolderOpen, Loader2, RefreshCw, Shield } from "lucide-vue-next";
import SettingsToggleRow from "../settings/SettingsToggleRow.vue";
import type { InlineFeedback } from "../settings/types";

const props = defineProps<{
  loading: boolean;
  hasSettings: boolean;
  backupDirectory: string;
  autoBackup: boolean;
  backupBeforeLaunch: boolean;
  compressionEnabled: boolean;
  compressionLevel: number;
  maxBackups: number;
  saving: boolean;
  saveDisabled: boolean;
  manifestRefreshing: boolean;
  manifestFeedback: InlineFeedback | null;
  saveFeedback: InlineFeedback | null;
}>();

const emit = defineEmits<{
  retry: [];
  chooseBackupDirectory: [];
  refreshManifest: [];
  updateBackupDirectory: [value: string];
  updateAutoBackup: [value: boolean];
  updateBackupBeforeLaunch: [value: boolean];
  updateCompressionEnabled: [value: boolean];
  updateCompressionLevel: [value: number];
  updateMaxBackups: [value: number];
  saveSettings: [];
}>();

function handleCompressionInput(event: Event) {
  const value = Number.parseInt((event.target as HTMLInputElement).value, 10);
  if (Number.isNaN(value)) {
    return;
  }

  emit("updateCompressionLevel", value);
}

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

  return feedback.tone === "error" ? "text-red-300" : "text-emerald-300";
}
</script>

<template>
  <section class="space-y-4">
    <div class="rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
      <div class="flex items-start justify-between gap-3">
        <div>
          <div class="text-sm font-semibold">SQOBA manifest</div>
          <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
            Refresh the local save-path manifest used for automatic detection and backups.
          </p>
        </div>
        <button
          type="button"
          class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="props.manifestRefreshing"
          @click="emit('refreshManifest')"
        >
          <Loader2 v-if="props.manifestRefreshing" class="h-4 w-4 animate-spin" />
          <RefreshCw v-else class="h-4 w-4" />
          Refresh
        </button>
      </div>

      <div
        v-if="props.manifestFeedback"
        class="mt-3 text-xs"
        :class="feedbackClass(props.manifestFeedback)"
      >
        {{ props.manifestFeedback.text }}
      </div>
    </div>

    <div class="rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
      <div class="mb-4 flex items-center gap-2">
        <Shield class="h-5 w-5" />
        <div>
          <h2 class="text-base font-semibold sm:text-lg">Backup settings</h2>
          <p class="text-xs text-muted-foreground">
            Backup directory, automation, and compression controls for SQOBA.
          </p>
        </div>
      </div>

      <div v-if="props.loading" class="flex min-h-48 items-center justify-center">
        <Loader2 class="h-6 w-6 animate-spin" />
      </div>

      <div
        v-else-if="!props.hasSettings"
        class="rounded-xl border border-red-500/30 bg-red-500/10 p-4 text-sm"
      >
        <div class="font-medium text-red-200">Settings could not be loaded.</div>
        <p class="mt-1 text-red-100/80">Retry once the local bridge is available.</p>
        <button
          type="button"
          class="mt-4 inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-4 py-2 text-sm transition-colors hover:bg-accent/70"
          @click="emit('retry')"
        >
          <RefreshCw class="h-4 w-4" />
          Retry
        </button>
      </div>

      <div v-else class="space-y-4">
        <div>
          <label class="mb-2 block text-sm font-medium" for="sqoba-backup-directory">
            Backup directory
          </label>
          <div class="flex gap-2">
            <input
              id="sqoba-backup-directory"
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

        <div class="space-y-2 rounded-xl border border-border/70 bg-background/35 p-3">
          <SettingsToggleRow
            id="sqoba-auto-backup"
            :model-value="props.autoBackup"
            label="Enable automatic backups"
            description="Create a backup automatically after the game exits."
            @update:model-value="emit('updateAutoBackup', $event)"
          />
          <SettingsToggleRow
            id="sqoba-backup-before-launch"
            :model-value="props.backupBeforeLaunch"
            label="Suggest a backup before launch"
            description="Prompt for a backup when save data changed before the next launch."
            @update:model-value="emit('updateBackupBeforeLaunch', $event)"
          />
        </div>

        <div class="space-y-3 rounded-xl border border-border/70 bg-background/35 p-3">
          <SettingsToggleRow
            id="sqoba-compression"
            :model-value="props.compressionEnabled"
            label="Enable compression"
            description="Compressed backups save space with a small speed tradeoff."
            @update:model-value="emit('updateCompressionEnabled', $event)"
          />

          <div class="space-y-3" :class="props.compressionEnabled ? '' : 'opacity-50'">
            <div class="flex items-center gap-3">
              <label class="text-xs text-muted-foreground" for="sqoba-compression-level">
                Compression level
              </label>
              <input
                id="sqoba-compression-level"
                type="number"
                min="1"
                max="100"
                :value="props.compressionLevel"
                :disabled="!props.compressionEnabled"
                class="h-10 w-20 rounded-xl border border-border/70 bg-card/70 px-3 text-sm outline-none disabled:cursor-not-allowed"
                @input="handleCompressionInput"
              />
              <div class="ml-auto text-xs text-muted-foreground">
                {{ props.compressionLevel }}
              </div>
            </div>

            <input
              type="range"
              min="1"
              max="100"
              :value="props.compressionLevel"
              :disabled="!props.compressionEnabled"
              class="w-full accent-primary disabled:cursor-not-allowed"
              @input="handleCompressionInput"
            />
          </div>
        </div>

        <div>
          <label class="mb-2 block text-sm font-medium" for="sqoba-max-backups">
            Max backups per game
          </label>
          <input
            id="sqoba-max-backups"
            type="number"
            min="1"
            max="100"
            :value="props.maxBackups"
            class="h-11 w-28 rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
            @input="handleMaxBackupsInput"
          />
          <p class="mt-1 text-xs text-muted-foreground">
            Older backups are pruned once the per-game limit is exceeded.
          </p>
        </div>

        <div class="flex flex-col gap-3 pt-1 sm:flex-row sm:items-center sm:justify-end">
          <div
            v-if="props.saveFeedback"
            class="text-sm"
            :class="feedbackClass(props.saveFeedback)"
          >
            {{ props.saveFeedback.text }}
          </div>
          <button
            type="button"
            class="inline-flex items-center justify-center gap-2 rounded-xl bg-primary px-4 py-3 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="props.saveDisabled"
            @click="emit('saveSettings')"
          >
            <Loader2 v-if="props.saving" class="h-4 w-4 animate-spin" />
            <Check v-else class="h-4 w-4" />
            Save settings
          </button>
        </div>
      </div>
    </div>
  </section>
</template>
