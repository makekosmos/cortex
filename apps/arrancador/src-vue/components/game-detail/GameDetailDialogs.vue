<script setup lang="ts">
import type { RawgGame } from "@/types";
import BackupProgressToast from "./BackupProgressToast.vue";
import GameDescriptionModal from "./GameDescriptionModal.vue";
import GameEditDialog from "./GameEditDialog.vue";
import GameMetadataSearchModal from "./GameMetadataSearchModal.vue";
import GameRatingModal from "./GameRatingModal.vue";

defineProps<{
  description: string | null;
  gameName: string;
  savingEdit: boolean;
  metadataResults: readonly RawgGame[];
  searchingMetadata: boolean;
  applyingMetadata: boolean;
  backupProgress: {
    active: boolean;
    stage: string;
    message: string;
    done: number;
    total: number;
  };
}>();

const emit = defineEmits<{
  saveRating: [rating: number];
  saveEdit: [];
  searchImage: [payload: { query: string; target: "background" | "cover" }];
  searchMetadata: [];
  applyMetadata: [result: RawgGame];
}>();

const descriptionOpen = defineModel<boolean>("descriptionOpen", { required: true });
const ratingOpen = defineModel<boolean>("ratingOpen", { required: true });
const ratingDraft = defineModel<number>("ratingDraft", { required: true });
const editOpen = defineModel<boolean>("editOpen", { required: true });
const editName = defineModel<string>("editName", { required: true });
const editDescription = defineModel<string>("editDescription", { required: true });
const editBackgroundImage = defineModel<string>("editBackgroundImage", { required: true });
const editCoverImage = defineModel<string>("editCoverImage", { required: true });
const metadataOpen = defineModel<boolean>("metadataOpen", { required: true });
const metadataQuery = defineModel<string>("metadataQuery", { required: true });
const metadataRename = defineModel<boolean>("metadataRename", { required: true });
</script>

<template>
  <GameDescriptionModal
    :open="descriptionOpen"
    :description="description"
    @close="descriptionOpen = false"
  />

  <GameRatingModal
    v-model:open="ratingOpen"
    v-model:rating-draft="ratingDraft"
    @save="emit('saveRating', $event)"
  />

  <GameEditDialog
    v-model:name="editName"
    v-model:description="editDescription"
    v-model:background-image="editBackgroundImage"
    v-model:cover-image="editCoverImage"
    :open="editOpen"
    :game-name="gameName"
    :saving="savingEdit"
    @close="editOpen = false"
    @save="emit('saveEdit')"
    @search-image="emit('searchImage', $event)"
  />

  <GameMetadataSearchModal
    v-model:open="metadataOpen"
    v-model:query="metadataQuery"
    v-model:rename="metadataRename"
    :results="metadataResults"
    :searching="searchingMetadata"
    :applying="applyingMetadata"
    @search="emit('searchMetadata')"
    @apply="emit('applyMetadata', $event)"
  />

  <BackupProgressToast :progress="backupProgress" />
</template>
