<script setup lang="ts">
import { computed, onMounted, shallowRef, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { gamesApi } from "../../src/lib/api";
import { translateGenreListToRu } from "../../src/lib/genres";
import type {
  Game,
  NewGameProcessBinding,
} from "../../src/types";
import GameBackLink from "../components/game-detail/GameBackLink.vue";
import GameDetailDialogs from "../components/game-detail/GameDetailDialogs.vue";
import GameDetailHeroSection from "../components/game-detail/GameDetailHeroSection.vue";
import GameDetailSections from "../components/game-detail/GameDetailSections.vue";
import GameMissingState from "../components/game-detail/GameMissingState.vue";
import { useGameBackups } from "../composables/useGameBackups";
import { useGameLaunchFlow } from "../composables/useGameLaunchFlow";
import { useGameMetadataSearch } from "../composables/useGameMetadataSearch";
import { useGameSavePath } from "../composables/useGameSavePath";
import { useGameStatus } from "../composables/useGameStatus";
import { useToast } from "../composables/useToast";
import { formatPlayedHours, normalizeDescription } from "../lib/gameDetailDisplay";
import { useGamesStore } from "../stores/games";

const route = useRoute();
const router = useRouter();
const gamesStore = useGamesStore();
const { notify } = useToast();

const routeGameId = computed(() =>
  typeof route.params.id === "string" ? route.params.id : "",
);
const game = computed(() => gamesStore.getGame(routeGameId.value) ?? null);

const savingNote = shallowRef(false);
const savingRating = shallowRef(false);
const addingProcessBindings = shallowRef(false);
const removingProcessBindingId = shallowRef<number | null>(null);

const showDescriptionModal = shallowRef(false);
const showRatingModal = shallowRef(false);

const userNote = shallowRef("");
const userRating = shallowRef<number | null>(null);
const ratingDraft = shallowRef(4);
const playStatus = shallowRef<Game["play_status"]>("not_started");

const {
  isInstalled,
  checkingInstalled,
  runningCount,
  checkingRunning,
  setRunningCount,
} = useGameStatus(() => game.value?.id, () => game.value?.exe_path);

const {
  backups,
  loadingBackups,
  restoring,
  creatingBackup,
  showAllBackups,
  backupProgress,
  latestBackup,
  olderBackups,
  loadBackups,
  createManualBackup,
  restoreBackup,
} = useGameBackups({
  game: () => game.value,
  refreshGames: gamesStore.refreshGames,
  notify,
});

const {
  savingPath,
  locatingSavePath,
  savePathDraft,
  savePathPreviewParts,
  canOpenSavePath,
  locateSavePath,
  openSavePath,
  chooseSaveFolder,
  chooseSaveFile,
  insertGamePathToken,
  saveGamePath,
} = useGameSavePath({
  game: () => game.value,
  updateGame: gamesStore.updateGame,
  notify,
});

const {
  searchingMetadata,
  applyingMetadata,
  savingEdit,
  showMetadataSearch,
  showEditDialog,
  metadataQuery,
  metadataResults,
  renameFromMetadata,
  editForm,
  handleSaveEdit,
  handleSearchGoogleImage,
  searchMetadata,
  applyMetadata,
} = useGameMetadataSearch({
  game: () => game.value,
  updateGame: gamesStore.updateGame,
  refreshGames: gamesStore.refreshGames,
});

const heroImage = computed(
  () => game.value?.background_image || game.value?.cover_image || null,
);
const heroGenres = computed(
  () => translateGenreListToRu(game.value?.genres, 3).join(" · ") || null,
);
const heroDescription = computed(() =>
  normalizeDescription(game.value?.description ?? null),
);
const heroReleaseYear = computed(() =>
  game.value?.released ? String(new Date(game.value.released).getFullYear()) : null,
);
const heroMeta = computed(() =>
  [heroReleaseYear.value, formatPlayedHours(game.value?.total_playtime ?? 0)]
    .filter(Boolean)
    .join(" · "),
);
const isMissing = computed(() => !isInstalled.value && !checkingInstalled.value);

const { launching, handleLaunch } = useGameLaunchFlow({
  game: () => game.value,
  isMissing: () => isMissing.value,
  runningCount,
  restoring,
  creatingBackup,
  setRunningCount,
  refreshGames: gamesStore.refreshGames,
  loadBackups,
  notify,
});

watch(
  game,
  (currentGame) => {
    if (!currentGame) return;
    userNote.value = currentGame.user_note || "";
    userRating.value = currentGame.user_rating ?? null;
    playStatus.value = currentGame.play_status || "not_started";
  },
  { immediate: true },
);

async function ensureGameLoaded() {
  if (game.value) return;
  if (gamesStore.games.length === 0) {
    await gamesStore.refreshGames();
  }
}

async function handleToggleFavorite() {
  if (!game.value) return;
  await gamesStore.toggleFavorite(game.value.id);
}

async function handleDelete() {
  if (!game.value) return;
  if (!window.confirm(`Удалить "${game.value.name}" из библиотеки?`)) return;
  await gamesStore.deleteGame(game.value.id);
  await router.push("/");
}

async function handleAddProcessBindings(bindings: NewGameProcessBinding[]) {
  if (!game.value || bindings.length === 0) return;

  addingProcessBindings.value = true;
  try {
    await gamesApi.addProcessBindings(game.value.id, bindings);
    await gamesStore.refreshGames();
    notify({
      tone: "success",
      title: `Добавлено привязок: ${bindings.length}`,
    });
  } catch (cause) {
    console.error("Failed to add process bindings:", cause);
    notify({
      tone: "error",
      title: "Не удалось добавить привязки процессов",
      description: cause instanceof Error ? cause.message : undefined,
    });
  } finally {
    addingProcessBindings.value = false;
  }
}

async function handleRemoveProcessBinding(bindingId: number) {
  if (!game.value) return;

  removingProcessBindingId.value = bindingId;
  try {
    await gamesApi.removeProcessBinding(game.value.id, bindingId);
    await gamesStore.refreshGames();
    notify({
      tone: "success",
      title: "Привязка процесса удалена",
    });
  } catch (cause) {
    console.error("Failed to remove process binding:", cause);
    notify({
      tone: "error",
      title: "Не удалось удалить привязку процесса",
      description: cause instanceof Error ? cause.message : undefined,
    });
  } finally {
    removingProcessBindingId.value = null;
  }
}

async function toggleBackupEnabled(next: boolean) {
  if (!game.value) return;
  try {
    await gamesStore.updateGame(game.value.id, {
      backup_enabled: next,
    });
  } catch (cause) {
    console.error("Failed to update backup setting:", cause);
    notify({
      tone: "error",
      title: "Не удалось обновить настройку бэкапов",
    });
  }
}

function openMetadataSearch() {
  if (!game.value) return;
  metadataQuery.value = game.value.name;
  showMetadataSearch.value = true;
}

function openRatingModal() {
  ratingDraft.value = userRating.value ?? 4;
  showRatingModal.value = true;
}

function saveRatingFromModal(rating: number) {
  userRating.value = rating;
  void saveUserRating();
}

async function savePlayStatus() {
  if (!game.value) return;
  try {
    await gamesStore.updateGame(game.value.id, {
      play_status: playStatus.value,
    });
  } catch (cause) {
    console.error("Failed to update play status:", cause);
  }
}

async function saveUserRating() {
  if (!game.value) return;
  savingRating.value = true;
  try {
    await gamesStore.updateGame(game.value.id, {
      user_rating: userRating.value,
    });
    notify({
      tone: "success",
      title: "Рейтинг сохранен",
    });
  } catch (cause) {
    console.error("Failed to update user rating:", cause);
  } finally {
    savingRating.value = false;
  }
}

async function saveUserNote() {
  if (!game.value) return;
  savingNote.value = true;
  try {
    await gamesStore.updateGame(game.value.id, {
      user_note: userNote.value,
    });
    notify({
      tone: "success",
      title: "Заметка сохранена",
    });
  } catch (cause) {
    console.error("Failed to update user note:", cause);
  } finally {
    savingNote.value = false;
  }
}

onMounted(async () => {
  await ensureGameLoaded();
});
</script>

<template>
  <GameMissingState v-if="!game" />

  <div v-else class="mx-auto max-w-6xl space-y-6 p-4 sm:p-6">
    <GameBackLink />

    <GameDetailHeroSection
      :game="game"
      :hero-image="heroImage"
      :hero-genres="heroGenres"
      :hero-description="heroDescription"
      :hero-meta="heroMeta"
      :launching="launching"
      :restoring="restoring"
      :is-missing="isMissing"
      :running-count="runningCount"
      :checking-running="checkingRunning"
      @launch="void handleLaunch()"
      @toggle-favorite="void handleToggleFavorite()"
      @show-description="showDescriptionModal = true"
    />

    <GameDetailSections
      v-model:play-status="playStatus"
      v-model:user-rating="userRating"
      v-model:user-note="userNote"
      v-model:save-path-draft="savePathDraft"
      v-model:show-all-backups="showAllBackups"
      :game="game"
      :saving-rating="savingRating"
      :saving-note="savingNote"
      :adding-process-bindings="addingProcessBindings"
      :removing-process-binding-id="removingProcessBindingId"
      :save-path-preview-parts="savePathPreviewParts"
      :backup-enabled="game.backup_enabled"
      :creating-backup="creatingBackup"
      :saving-path="savingPath"
      :locating-save-path="locatingSavePath"
      :can-open-save-path="canOpenSavePath"
      :backups="backups"
      :latest-backup="latestBackup"
      :older-backups="olderBackups"
      :loading-backups="loadingBackups"
      :restoring="restoring"
      @save-play-status="void savePlayStatus()"
      @save-user-rating="void saveUserRating()"
      @open-rating-modal="openRatingModal"
      @save-user-note="void saveUserNote()"
      @edit="showEditDialog = true"
      @add-process-bindings="void handleAddProcessBindings($event)"
      @remove-process-binding="void handleRemoveProcessBinding($event)"
      @toggle-backup-enabled="void toggleBackupEnabled($event)"
      @create-backup="void createManualBackup()"
      @choose-save-folder="void chooseSaveFolder()"
      @choose-save-file="void chooseSaveFile()"
      @insert-game-path-token="insertGamePathToken()"
      @locate-save-path="void locateSavePath()"
      @open-save-path="void openSavePath()"
      @save-game-path="void saveGamePath()"
      @restore-backup="void restoreBackup($event)"
      @search-rawg="openMetadataSearch"
      @delete="void handleDelete()"
    />

    <GameDetailDialogs
      v-model:description-open="showDescriptionModal"
      v-model:rating-open="showRatingModal"
      v-model:rating-draft="ratingDraft"
      v-model:edit-open="showEditDialog"
      v-model:edit-name="editForm.name"
      v-model:edit-description="editForm.description"
      v-model:edit-background-image="editForm.background_image"
      v-model:edit-cover-image="editForm.cover_image"
      v-model:metadata-open="showMetadataSearch"
      v-model:metadata-query="metadataQuery"
      v-model:metadata-rename="renameFromMetadata"
      :description="heroDescription"
      :game-name="game.name"
      :saving-edit="savingEdit"
      :metadata-results="metadataResults"
      :searching-metadata="searchingMetadata"
      :applying-metadata="applyingMetadata"
      :backup-progress="backupProgress"
      @save-rating="saveRatingFromModal"
      @save-edit="void handleSaveEdit()"
      @search-image="void handleSearchGoogleImage($event.query, $event.target)"
      @search-metadata="void searchMetadata()"
      @apply-metadata="void applyMetadata($event)"
    />
  </div>
</template>
