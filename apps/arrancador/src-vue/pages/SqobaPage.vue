<script setup lang="ts">
import { Sparkles } from "lucide-vue-next";
import SqobaAboutModal from "../components/sqoba/SqobaAboutModal.vue";
import SqobaSavesPanel from "../components/sqoba/SqobaSavesPanel.vue";
import SqobaSettingsPanel from "../components/sqoba/SqobaSettingsPanel.vue";
import { useSqobaPageState } from "../composables/useSqobaPageState";

const {
  settingsState,
  aboutOpen,
  query,
  onlyMissing,
  scanAllLoading,
  editingGameId,
  savePathDraft,
  savingSavePath,
  pathsByGameId,
  filesByGameId,
  games,
  gamesLoading,
  gamesError,
  filteredGames,
  missingCount,
  openAbout,
  closeAbout,
  openGamePath,
  loadSavePaths,
  loadSaveFiles,
  beginEdit,
  cancelEdit,
  selectSaveFolder,
  selectSaveFile,
  insertGamePathToken,
  saveGameSavePath,
  runScanAll,
  refreshManifestAndGames,
} = useSqobaPageState();

const {
  form,
  loading: settingsLoading,
  saving: settingsSaving,
  hasSettings,
  manifestRefreshing,
  manifestFeedback,
  saveFeedback,
  saveDisabled,
  loadSettings,
  setCompressionEnabled,
  setCompressionLevel,
  setMaxBackups,
  selectBackupDirectory,
  saveSettings,
} = settingsState;

function handleQueryChange(value: string) {
  query.value = value;
}

function handleOnlyMissingChange(value: boolean) {
  onlyMissing.value = value;
}

function handleSavePathDraftChange(value: string) {
  savePathDraft.value = value;
}
</script>

<template>
  <div class="mx-auto max-w-6xl px-4 py-4 sm:px-6 sm:py-6">
    <div class="mb-6 flex items-start justify-between gap-4">
      <div class="min-w-0">
        <h1 class="text-xl font-bold tracking-tight sm:text-2xl">SQOBA</h1>
        <p class="text-sm text-muted-foreground">
          Save discovery, manifest refresh, and backup controls in one place.
        </p>
      </div>

      <button
        type="button"
        class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-4 py-2 text-sm transition-colors hover:bg-accent/70"
        @click="openAbout"
      >
        <Sparkles class="h-4 w-4" />
        What is SQOBA?
      </button>
    </div>

    <div class="grid gap-4 xl:grid-cols-[340px,minmax(0,1fr)]">
      <SqobaSettingsPanel
        :loading="settingsLoading"
        :has-settings="hasSettings"
        :backup-directory="form.backupDirectory"
        :auto-backup="form.autoBackup"
        :backup-before-launch="form.backupBeforeLaunch"
        :compression-enabled="form.compressionEnabled"
        :compression-level="form.compressionLevel"
        :max-backups="form.maxBackups"
        :saving="settingsSaving"
        :save-disabled="saveDisabled"
        :manifest-refreshing="manifestRefreshing"
        :manifest-feedback="manifestFeedback"
        :save-feedback="saveFeedback"
        @retry="void loadSettings()"
        @choose-backup-directory="void selectBackupDirectory()"
        @refresh-manifest="void refreshManifestAndGames()"
        @update-backup-directory="form.backupDirectory = $event"
        @update-auto-backup="form.autoBackup = $event"
        @update-backup-before-launch="form.backupBeforeLaunch = $event"
        @update-compression-enabled="setCompressionEnabled"
        @update-compression-level="setCompressionLevel"
        @update-max-backups="setMaxBackups"
        @save-settings="void saveSettings()"
      />

      <SqobaSavesPanel
        :games="filteredGames"
        :games-loading="gamesLoading"
        :games-error="gamesError"
        :query="query"
        :only-missing="onlyMissing"
        :total-games="games.length"
        :missing-count="missingCount"
        :scan-all-loading="scanAllLoading"
        :paths-by-game-id="pathsByGameId"
        :files-by-game-id="filesByGameId"
        :editing-game-id="editingGameId"
        :save-path-draft="savePathDraft"
        :saving-save-path="savingSavePath"
        @update-query="handleQueryChange"
        @update-only-missing="handleOnlyMissingChange"
        @scan-all="void runScanAll()"
        @lookup-paths="void loadSavePaths($event)"
        @load-files="void loadSaveFiles($event)"
        @begin-edit="beginEdit"
        @cancel-edit="cancelEdit"
        @save-path-draft-change="handleSavePathDraftChange"
        @choose-save-folder="void selectSaveFolder()"
        @choose-save-file="void selectSaveFile()"
        @insert-game-path-token="insertGamePathToken"
        @open-path="void openGamePath($event.game, $event.path)"
        @save-game-path="void saveGameSavePath($event)"
      />
    </div>

    <SqobaAboutModal :open="aboutOpen" @close="closeAbout" />
  </div>
</template>
