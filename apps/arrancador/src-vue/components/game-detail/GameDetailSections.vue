<script setup lang="ts">
import type { Backup, Game, NewGameProcessBinding } from "@/types";
import GameDangerZone from "./GameDangerZone.vue";
import GameDetailBackupSection from "./GameDetailBackupSection.vue";
import GameDetailInfoSection from "./GameDetailInfoSection.vue";
import GameDetailMetadataSection from "./GameDetailMetadataSection.vue";
import GameDetailStateSection from "./GameDetailStateSection.vue";
import GameProcessBindingsSection from "./GameProcessBindingsSection.vue";

defineProps<{
  game: Game;
  savingRating: boolean;
  savingNote: boolean;
  addingProcessBindings: boolean;
  removingProcessBindingId: number | null;
  savePathPreviewParts: string[];
  backupEnabled: boolean;
  creatingBackup: boolean;
  savingPath: boolean;
  locatingSavePath: boolean;
  canOpenSavePath: boolean;
  backups: Backup[];
  latestBackup: Backup | null;
  olderBackups: Backup[];
  loadingBackups: boolean;
  restoring: boolean;
}>();

const emit = defineEmits<{
  savePlayStatus: [];
  saveUserRating: [];
  openRatingModal: [];
  saveUserNote: [];
  edit: [];
  addProcessBindings: [bindings: NewGameProcessBinding[]];
  removeProcessBinding: [bindingId: number];
  toggleBackupEnabled: [enabled: boolean];
  createBackup: [];
  chooseSaveFolder: [];
  chooseSaveFile: [];
  insertGamePathToken: [];
  locateSavePath: [];
  openSavePath: [];
  saveGamePath: [];
  restoreBackup: [backupId: string];
  searchRawg: [];
  delete: [];
}>();

const playStatus = defineModel<Game["play_status"]>("playStatus", { required: true });
const userRating = defineModel<number | null>("userRating", { required: true });
const userNote = defineModel<string>("userNote", { required: true });
const savePathDraft = defineModel<string>("savePathDraft", { required: true });
const showAllBackups = defineModel<boolean>("showAllBackups", { required: true });
</script>

<template>
  <div class="grid gap-6 xl:grid-cols-[minmax(0,1.2fr)_minmax(0,0.8fr)]">
    <section class="space-y-6">
      <GameDetailStateSection
        v-model:play-status="playStatus"
        v-model:user-rating="userRating"
        v-model:user-note="userNote"
        :total-playtime="game.total_playtime"
        :saving-rating="savingRating"
        :saving-note="savingNote"
        @save-play-status="emit('savePlayStatus')"
        @save-user-rating="emit('saveUserRating')"
        @open-rating-modal="emit('openRatingModal')"
        @save-user-note="emit('saveUserNote')"
      />

      <GameDetailInfoSection :game="game" @edit="emit('edit')" />

      <GameProcessBindingsSection
        :primary-exe-path="game.exe_path"
        :bindings="game.process_bindings"
        :adding="addingProcessBindings"
        :removing-binding-id="removingProcessBindingId"
        @add-bindings="emit('addProcessBindings', $event)"
        @remove-binding="emit('removeProcessBinding', $event)"
      />

      <GameDetailBackupSection
        v-model:save-path-draft="savePathDraft"
        v-model:show-all-backups="showAllBackups"
        :backup-enabled="backupEnabled"
        :creating-backup="creatingBackup"
        :saving-path="savingPath"
        :locating-save-path="locatingSavePath"
        :can-open-save-path="canOpenSavePath"
        :save-path-preview-parts="savePathPreviewParts"
        :backups="backups"
        :latest-backup="latestBackup"
        :older-backups="olderBackups"
        :loading-backups="loadingBackups"
        :restoring="restoring"
        @toggle-backup-enabled="emit('toggleBackupEnabled', $event)"
        @create-backup="emit('createBackup')"
        @choose-save-folder="emit('chooseSaveFolder')"
        @choose-save-file="emit('chooseSaveFile')"
        @insert-game-path-token="emit('insertGamePathToken')"
        @locate-save-path="emit('locateSavePath')"
        @open-save-path="emit('openSavePath')"
        @save-game-path="emit('saveGamePath')"
        @restore-backup="emit('restoreBackup', $event)"
      />
    </section>

    <section class="space-y-6">
      <GameDetailMetadataSection
        :game="game"
        @search-rawg="emit('searchRawg')"
        @edit="emit('edit')"
      />

      <GameDangerZone @delete="emit('delete')" />
    </section>
  </div>
</template>
