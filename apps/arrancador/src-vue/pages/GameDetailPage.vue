<script setup lang="ts">
import {
  ArrowLeft,
  Trash2,
} from "lucide-vue-next";
import { computed, onMounted, shallowRef, watch } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import { backupApi, gamesApi } from "../../src/lib/api";
import { translateGenreListToRu } from "../../src/lib/genres";
import type {
  Game,
  NewGameProcessBinding,
} from "../../src/types";
import BackupProgressToast from "../components/game-detail/BackupProgressToast.vue";
import GameDescriptionModal from "../components/game-detail/GameDescriptionModal.vue";
import GameDetailBackupSection from "../components/game-detail/GameDetailBackupSection.vue";
import GameDetailHeroSection from "../components/game-detail/GameDetailHeroSection.vue";
import GameDetailInfoSection from "../components/game-detail/GameDetailInfoSection.vue";
import GameDetailMetadataSection from "../components/game-detail/GameDetailMetadataSection.vue";
import GameDetailStateSection from "../components/game-detail/GameDetailStateSection.vue";
import GameEditDialog from "../components/game-detail/GameEditDialog.vue";
import GameMetadataSearchModal from "../components/game-detail/GameMetadataSearchModal.vue";
import GameProcessBindingsSection from "../components/game-detail/GameProcessBindingsSection.vue";
import GameRatingModal from "../components/game-detail/GameRatingModal.vue";
import { useGameBackups } from "../composables/useGameBackups";
import { useGameMetadataSearch } from "../composables/useGameMetadataSearch";
import { useGameSavePath } from "../composables/useGameSavePath";
import { useGameStatus } from "../composables/useGameStatus";
import { useToast } from "../composables/useToast";
import {
  formatBytes,
  formatPlayedHours,
  normalizeDescription,
} from "../lib/gameDetailDisplay";
import { useGamesStore } from "../stores/games";

const route = useRoute();
const router = useRouter();
const gamesStore = useGamesStore();
const { notify } = useToast();

const routeGameId = computed(() =>
  typeof route.params.id === "string" ? route.params.id : "",
);
const game = computed(() => gamesStore.getGame(routeGameId.value) ?? null);

const launching = shallowRef(false);
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

async function launchGame() {
  if (!game.value) return;
  launching.value = true;

  try {
    await gamesApi.launch(game.value.id);
    const count = await gamesApi.getRunningInstances(game.value.id);
    setRunningCount(count);
    await gamesStore.refreshGames();
  } catch (cause) {
    console.error("Failed to launch game:", cause);
    notify({
      tone: "error",
      title: "Не удалось запустить игру",
    });
  } finally {
    launching.value = false;
  }
}

async function handleLaunch() {
  if (!game.value || isMissing.value) return;

  if (runningCount.value > 0) {
    const confirmed = window.confirm(
      runningCount.value > 1
        ? `Игра уже запущена. Закрыть ${runningCount.value} процесса?`
        : "Игра уже запущена. Закрыть игру?",
    );

    if (!confirmed) return;

    try {
      await gamesApi.killProcesses(game.value.id);
      const remaining = await gamesApi.getRunningInstances(game.value.id);
      setRunningCount(remaining);
      if (remaining > 0) {
        notify({
          tone: "warning",
          title: "Не удалось закрыть все процессы",
          description: `Осталось процессов: ${remaining}`,
        });
      }
    } catch (cause) {
      console.error("Failed to kill processes:", cause);
      notify({
        tone: "error",
        title: "Не удалось закрыть игру",
      });
    }
    return;
  }

  let canLaunch = true;

  try {
    if (game.value.backup_enabled) {
      const restoreCheck = await backupApi.checkRestoreNeeded(
        game.value.id,
        game.value.name,
      );
      if (restoreCheck.should_restore && restoreCheck.backup_id) {
        const shouldRestore = window.confirm(
          `Текущий размер сохранений меньше, чем в бэкапе. Восстановить перед запуском?\n\nТекущий: ${formatBytes(restoreCheck.current_size)} • Бэкап: ${formatBytes(restoreCheck.backup_size)}`,
        );

        if (shouldRestore) {
          restoring.value = true;
          await backupApi.restore(restoreCheck.backup_id);
          await gamesStore.refreshGames();
          restoring.value = false;
        }
      }

      const shouldBackup = await backupApi.shouldBackupBeforeLaunch(game.value.id);
      if (shouldBackup) {
        const needsBackup = await backupApi.checkBackupNeeded(
          game.value.id,
          game.value.name,
        );
        if (needsBackup) {
          const shouldCreateBackup = window.confirm(
            "Сохранения изменились. Создать бэкап перед запуском?",
          );
          if (shouldCreateBackup) {
            creatingBackup.value = true;
            await backupApi.create(game.value.id, game.value.name, true);
            await loadBackups();
            await gamesStore.refreshGames();
            creatingBackup.value = false;
          }
        }
      }
    }
  } catch (cause) {
    console.error("Backup or restore pre-launch flow failed:", cause);
    creatingBackup.value = false;
    restoring.value = false;
    canLaunch = false;
    notify({
      tone: "error",
      title: "Не удалось подготовить запуск",
    });
  }

  if (!canLaunch) return;
  await launchGame();
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
  <div v-if="!game" class="flex h-full items-center justify-center p-6">
    <div class="text-center">
      <div class="text-lg font-semibold">Игра не найдена</div>
      <RouterLink to="/" class="mt-3 inline-flex text-primary hover:underline">
        Вернуться в библиотеку
      </RouterLink>
    </div>
  </div>

  <div v-else class="mx-auto max-w-6xl space-y-6 p-4 sm:p-6">
    <RouterLink
      to="/"
      class="inline-flex items-center gap-2 text-sm text-muted-foreground transition-colors hover:text-foreground"
    >
      <ArrowLeft class="h-4 w-4" />
      Назад
    </RouterLink>

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

    <div class="grid gap-6 xl:grid-cols-[minmax(0,1.2fr)_minmax(0,0.8fr)]">
      <section class="space-y-6">
        <GameDetailStateSection
          v-model:play-status="playStatus"
          v-model:user-rating="userRating"
          v-model:user-note="userNote"
          :total-playtime="game.total_playtime"
          :saving-rating="savingRating"
          :saving-note="savingNote"
          @save-play-status="void savePlayStatus()"
          @save-user-rating="void saveUserRating()"
          @open-rating-modal="openRatingModal"
          @save-user-note="void saveUserNote()"
        />

        <GameDetailInfoSection :game="game" @edit="showEditDialog = true" />

        <GameProcessBindingsSection
          :primary-exe-path="game.exe_path"
          :bindings="game.process_bindings"
          :adding="addingProcessBindings"
          :removing-binding-id="removingProcessBindingId"
          @add-bindings="void handleAddProcessBindings($event)"
          @remove-binding="void handleRemoveProcessBinding($event)"
        />

        <GameDetailBackupSection
          v-model:save-path-draft="savePathDraft"
          v-model:show-all-backups="showAllBackups"
          :backup-enabled="game.backup_enabled"
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
          @toggle-backup-enabled="void toggleBackupEnabled($event)"
          @create-backup="void createManualBackup()"
          @choose-save-folder="void chooseSaveFolder()"
          @choose-save-file="void chooseSaveFile()"
          @insert-game-path-token="insertGamePathToken()"
          @locate-save-path="void locateSavePath()"
          @open-save-path="void openSavePath()"
          @save-game-path="void saveGamePath()"
          @restore-backup="void restoreBackup($event)"
        />
      </section>

      <section class="space-y-6">
        <GameDetailMetadataSection
          :game="game"
          @search-rawg="openMetadataSearch"
          @edit="showEditDialog = true"
        />

        <div class="rounded-2xl border border-border/70 bg-card/80 p-4 shadow-sm">
          <div class="mb-4 flex items-center justify-between gap-3">
            <div>
              <div class="text-base font-semibold">Опасные действия</div>
              <div class="text-xs text-muted-foreground">
                Удаление записи из библиотеки
              </div>
            </div>
          </div>

          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-red-500/35 bg-red-500/10 px-4 py-2 text-sm text-red-300 transition-colors hover:bg-red-500/15"
            @click="void handleDelete()"
          >
            <Trash2 class="h-4 w-4" />
            Удалить игру
          </button>
        </div>
      </section>
    </div>

    <GameDescriptionModal
      :open="showDescriptionModal"
      :description="heroDescription"
      @close="showDescriptionModal = false"
    />

    <GameRatingModal
      v-model:open="showRatingModal"
      v-model:rating-draft="ratingDraft"
      @save="saveRatingFromModal"
    />

    <GameEditDialog
      v-model:name="editForm.name"
      v-model:description="editForm.description"
      v-model:background-image="editForm.background_image"
      v-model:cover-image="editForm.cover_image"
      :open="showEditDialog"
      :game-name="game.name"
      :saving="savingEdit"
      @close="showEditDialog = false"
      @save="void handleSaveEdit()"
      @search-image="void handleSearchGoogleImage($event.query, $event.target)"
    />

    <GameMetadataSearchModal
      v-model:open="showMetadataSearch"
      v-model:query="metadataQuery"
      v-model:rename="renameFromMetadata"
      :results="metadataResults"
      :searching="searchingMetadata"
      :applying="applyingMetadata"
      @search="void searchMetadata()"
      @apply="void applyMetadata($event)"
    />

    <BackupProgressToast :progress="backupProgress" />
  </div>
</template>
