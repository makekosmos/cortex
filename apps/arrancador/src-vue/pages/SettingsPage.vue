<script setup lang="ts">
import { Check, Loader2, RefreshCw } from "lucide-vue-next";
import { computed, shallowRef, useTemplateRef } from "vue";
import AppearanceSettingsSection from "../components/settings/AppearanceSettingsSection.vue";
import BackupSettingsSection from "../components/settings/BackupSettingsSection.vue";
import CompressionSettingsSection from "../components/settings/CompressionSettingsSection.vue";
import RawgSettingsSection from "../components/settings/RawgSettingsSection.vue";
import SettingsSectionNav from "../components/settings/SettingsSectionNav.vue";
import SqobaSettingsSection from "../components/settings/SqobaSettingsSection.vue";
import SystemSettingsSection from "../components/settings/SystemSettingsSection.vue";
import type { SettingsSectionId, SettingsSectionItem } from "../components/settings/types";
import { useSettingsPageState } from "../composables/useSettingsPageState";

const pageTopRef = useTemplateRef<HTMLDivElement>("pageTop");
const activeSection = shallowRef<SettingsSectionId>("all");

const {
  theme,
  form,
  loading,
  saving,
  hasSettings,
  autoStart,
  autoStartPending,
  manifestRefreshing,
  manifestFeedback,
  saveFeedback,
  saveDisabled,
  setTheme,
  loadSettings,
  toggleAutoStart,
  setCompressionEnabled,
  setCompressionLevel,
  setMaxBackups,
  selectBackupDirectory,
  refreshSqobaManifest,
  saveSettings,
} = useSettingsPageState();

const sectionItems: SettingsSectionItem[] = [
  { id: "all", label: "All", icon: "monitor" },
  { id: "appearance", label: "Appearance", icon: "monitor" },
  { id: "system", label: "System", icon: "power" },
  { id: "backup", label: "Backups", icon: "shield" },
  { id: "compression", label: "Compression", icon: "hardDrive" },
  { id: "sqoba", label: "SQOBA", icon: "sparkles" },
  { id: "rawg", label: "RAWG", icon: "key" },
];

const saveFeedbackClass = computed(() => {
  if (!saveFeedback.value) {
    return "";
  }

  return saveFeedback.value.tone === "error"
    ? "text-red-300"
    : "text-emerald-300";
});

function handleSectionSelect(sectionId: SettingsSectionId) {
  activeSection.value = sectionId;

  if (sectionId === "all") {
    pageTopRef.value?.scrollIntoView({ behavior: "smooth", block: "start" });
    return;
  }

  window.document
    .getElementById(`settings-${sectionId}`)
    ?.scrollIntoView({ behavior: "smooth", block: "start" });
}
</script>

<template>
  <div ref="pageTop" class="mx-auto max-w-5xl px-4 py-4 sm:px-6 sm:py-6">
    <div class="mb-6 space-y-2 sm:mb-8">
      <h1 class="text-xl font-bold tracking-tight sm:text-2xl">Settings</h1>
      <p class="text-sm text-muted-foreground">
        Configure Arrancador backup, metadata, and system preferences.
      </p>
    </div>

    <div class="mb-6">
      <SettingsSectionNav
        :items="sectionItems"
        :active-section="activeSection"
        @select="handleSectionSelect"
      />
    </div>

    <div v-if="loading" class="flex min-h-[40vh] items-center justify-center p-6">
      <Loader2 class="h-8 w-8 animate-spin" />
    </div>

    <div
      v-else-if="!hasSettings"
      class="rounded-2xl border border-red-500/30 bg-red-500/10 p-4 text-sm"
    >
      <div class="font-medium text-red-200">Settings could not be loaded.</div>
      <p class="mt-1 text-red-100/80">
        Retry after the bridge and backend are available.
      </p>
      <button
        type="button"
        class="mt-4 inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-4 py-2 text-sm transition-colors hover:bg-accent/70"
        @click="void loadSettings()"
      >
        <RefreshCw class="h-4 w-4" />
        Retry
      </button>
    </div>

    <div v-else class="space-y-6 pb-20 sm:space-y-8 sm:pb-0">
      <AppearanceSettingsSection
        section-id="settings-appearance"
        :theme="theme"
        @update-theme="setTheme"
      />

      <SystemSettingsSection
        section-id="settings-system"
        :auto-start="autoStart"
        :pending="autoStartPending"
        @update-auto-start="void toggleAutoStart($event)"
      />

      <BackupSettingsSection
        section-id="settings-backup"
        :backup-directory="form.backupDirectory"
        :max-backups="form.maxBackups"
        :auto-backup="form.autoBackup"
        :backup-before-launch="form.backupBeforeLaunch"
        :manifest-refreshing="manifestRefreshing"
        :manifest-feedback="manifestFeedback"
        @choose-backup-directory="void selectBackupDirectory()"
        @refresh-manifest="void refreshSqobaManifest()"
        @update-backup-directory="form.backupDirectory = $event"
        @update-max-backups="setMaxBackups"
        @update-auto-backup="form.autoBackup = $event"
        @update-backup-before-launch="form.backupBeforeLaunch = $event"
      />

      <CompressionSettingsSection
        section-id="settings-compression"
        :compression-enabled="form.compressionEnabled"
        :compression-level="form.compressionLevel"
        :skip-compression-once="form.skipCompressionOnce"
        @update-compression-enabled="setCompressionEnabled"
        @update-compression-level="setCompressionLevel"
        @update-skip-compression-once="form.skipCompressionOnce = $event"
      />

      <SqobaSettingsSection
        section-id="settings-sqoba"
        :manifest-refreshing="manifestRefreshing"
        :manifest-feedback="manifestFeedback"
        @refresh-manifest="void refreshSqobaManifest()"
      />

      <RawgSettingsSection
        section-id="settings-rawg"
        :rawg-api-key="form.rawgApiKey"
        @update-rawg-api-key="form.rawgApiKey = $event"
      />

      <div class="fixed inset-x-0 bottom-0 z-10 border-t border-border/70 bg-background/85 p-4 backdrop-blur-md sm:static sm:border-none sm:bg-transparent sm:p-0">
        <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-end">
          <div
            v-if="saveFeedback"
            class="text-sm"
            :class="saveFeedbackClass"
          >
            {{ saveFeedback.text }}
          </div>
          <button
            type="button"
            class="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-primary px-4 py-3 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60 sm:w-auto"
            :disabled="saveDisabled"
            @click="void saveSettings()"
          >
            <Loader2 v-if="saving" class="h-4 w-4 animate-spin" />
            <Check v-else class="h-4 w-4" />
            Save settings
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
