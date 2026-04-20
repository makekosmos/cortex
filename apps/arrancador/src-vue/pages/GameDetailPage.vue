<script setup lang="ts">
import {
  Activity,
  ArrowLeft,
  ExternalLink,
  File as FileIcon,
  FolderOpen,
  Gamepad2,
  Globe,
  Heart,
  HardDrive,
  Image as ImageIcon,
  Loader2,
  Pencil,
  Play,
  Save,
  Search,
  Shield,
  Star,
  Timer,
  Trash2,
  X,
} from "lucide-vue-next";
import { computed, onMounted, reactive, shallowRef, watch } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import { backupApi, gamesApi, metadataApi } from "../../src/lib/api";
import {
  openPath,
  pickDirectoryPath,
  pickFilePath,
  subscribeAppEvent,
} from "../../src/lib/browser";
import { translateGenreListToRu } from "../../src/lib/genres";
import type { Backup, Game, RawgGame } from "../../src/types";
import { useGameStatus } from "../composables/useGameStatus";
import { useToast } from "../composables/useToast";
import { useGamesStore } from "../stores/games";

type BackupProgressPayload = {
  game_id: string;
  stage: string;
  message: string;
  done: number;
  total: number;
};

const GAME_PATH_TOKEN = "{PATHTOGAME}";

function formatPlaytime(seconds: number) {
  if (!seconds) return "0 ч";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) return `${hours} ч ${minutes} мин`;
  return `${minutes} мин`;
}

function formatPlayedHours(seconds: number) {
  if (seconds <= 0) return "0 ч";
  if (seconds < 3600) return "<1 ч";
  return `${Math.floor(seconds / 3600)} ч`;
}

function formatBytes(bytes: number) {
  if (!bytes || bytes <= 0) return "0 Б";
  const units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  let size = bytes;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  const digits = size >= 10 ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unitIndex]}`;
}

function normalizeDescription(value: string | null) {
  if (!value) return null;
  const plain = value.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
  return plain || null;
}

function resolveSavePathTemplate(path: string, game?: Game | null) {
  if (!path.includes(GAME_PATH_TOKEN)) return path;
  const exePath = game?.exe_path;
  if (!exePath) return path;
  const lastSlash = Math.max(exePath.lastIndexOf("\\"), exePath.lastIndexOf("/"));
  const base = lastSlash > 0 ? exePath.slice(0, lastSlash) : exePath;
  return path.split(GAME_PATH_TOKEN).join(base);
}

const PLAY_STATUS_LABELS: Record<Game["play_status"], string> = {
  not_started: "Не начато",
  in_progress: "В процессе",
  completed: "Пройдено",
  abandoned: "Брошено",
};

const PLAY_STATUS_TONES: Record<Game["play_status"], string> = {
  not_started: "border-white/10 text-muted-foreground",
  in_progress: "border-sky-400/35 text-sky-300",
  completed: "border-emerald-400/35 text-emerald-300",
  abandoned: "border-rose-400/35 text-rose-300",
};

const route = useRoute();
const router = useRouter();
const gamesStore = useGamesStore();
const { notify } = useToast();

const routeGameId = computed(() =>
  typeof route.params.id === "string" ? route.params.id : "",
);
const game = computed(() => gamesStore.getGame(routeGameId.value) ?? null);

const backups = shallowRef<Backup[]>([]);
const loadingBackups = shallowRef(false);
const launching = shallowRef(false);
const restoring = shallowRef(false);
const creatingBackup = shallowRef(false);
const savingNote = shallowRef(false);
const savingRating = shallowRef(false);
const savingPath = shallowRef(false);
const locatingSavePath = shallowRef(false);
const searchingMetadata = shallowRef(false);
const applyingMetadata = shallowRef(false);
const savingEdit = shallowRef(false);

const showMetadataSearch = shallowRef(false);
const showEditDialog = shallowRef(false);
const showDescriptionModal = shallowRef(false);
const showRatingModal = shallowRef(false);
const showAllBackups = shallowRef(false);

const metadataQuery = shallowRef("");
const metadataResults = shallowRef<RawgGame[]>([]);
const renameFromMetadata = shallowRef(false);
const userNote = shallowRef("");
const userRating = shallowRef<number | null>(null);
const ratingDraft = shallowRef(4);
const playStatus = shallowRef<Game["play_status"]>("not_started");
const savePathDraft = shallowRef("");

const editForm = reactive({
  name: "",
  description: "",
  background_image: "",
  cover_image: "",
});

const backupProgress = reactive({
  active: false,
  stage: "",
  message: "",
  done: 0,
  total: 0,
});

const {
  isInstalled,
  checkingInstalled,
  runningCount,
  checkingRunning,
  setRunningCount,
} = useGameStatus(() => game.value?.id, () => game.value?.exe_path);

const latestBackup = computed(() => backups.value[0] ?? null);
const olderBackups = computed(() => backups.value.slice(1));
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
const savePathPreviewParts = computed(() => savePathDraft.value.split(GAME_PATH_TOKEN));
const canOpenSavePath = computed(() => savePathDraft.value.trim().length > 0);
const isMissing = computed(() => !isInstalled.value && !checkingInstalled.value);
const displayRating = computed(() =>
  showRatingModal.value ? ratingDraft.value : userRating.value,
);

watch(
  game,
  (currentGame) => {
    if (!currentGame) return;
    editForm.name = currentGame.name;
    editForm.description = currentGame.description || "";
    editForm.background_image = currentGame.background_image || "";
    editForm.cover_image = currentGame.cover_image || "";
    userNote.value = currentGame.user_note || "";
    userRating.value = currentGame.user_rating ?? null;
    playStatus.value = currentGame.play_status || "not_started";
    savePathDraft.value = currentGame.save_path || "";
  },
  { immediate: true },
);

watch(
  () => game.value?.id,
  async (currentId) => {
    if (!currentId) {
      backups.value = [];
      return;
    }
    await loadBackups();
  },
  { immediate: true },
);

watch(
  () => game.value?.id,
  (currentId, _previousId, onCleanup) => {
    if (!currentId) return;

    const syncProgress = (payload: BackupProgressPayload) => {
      if (payload.game_id !== currentId) return;
      backupProgress.stage = payload.stage;
      backupProgress.message = payload.message;
      backupProgress.done = payload.done;
      backupProgress.total = payload.total;
      backupProgress.active = payload.stage !== "done";
    };

    const stopBackup = subscribeAppEvent<BackupProgressPayload>(
      "backup:progress",
      syncProgress,
    );
    const stopRestore = subscribeAppEvent<BackupProgressPayload>(
      "restore:progress",
      syncProgress,
    );

    onCleanup(() => {
      stopBackup();
      stopRestore();
    });
  },
  { immediate: true },
);

async function ensureGameLoaded() {
  if (game.value) return;
  if (gamesStore.games.length === 0) {
    await gamesStore.refreshGames();
  }
}

async function loadBackups() {
  if (!game.value) return;
  loadingBackups.value = true;
  try {
    backups.value = [...(await backupApi.getForGame(game.value.id))].sort(
      (left, right) =>
        new Date(right.created_at).getTime() - new Date(left.created_at).getTime(),
    );
  } catch (cause) {
    console.error("Failed to load backups:", cause);
  } finally {
    loadingBackups.value = false;
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

async function createManualBackup() {
  if (!game.value) return;
  creatingBackup.value = true;
  try {
    await backupApi.create(game.value.id, game.value.name, false);
    await loadBackups();
    await gamesStore.refreshGames();
    notify({
      tone: "success",
      title: "Бэкап создан",
    });
  } catch (cause) {
    console.error("Backup failed:", cause);
    notify({
      tone: "error",
      title: "Не удалось создать бэкап",
    });
  } finally {
    creatingBackup.value = false;
  }
}

async function restoreBackup(backupId: string) {
  if (
    !window.confirm(
      "Восстановить бэкап? Текущие сохранения будут перезаписаны.",
    )
  ) {
    return;
  }

  restoring.value = true;
  try {
    await backupApi.restore(backupId);
    await loadBackups();
    await gamesStore.refreshGames();
    notify({
      tone: "success",
      title: "Бэкап восстановлен",
    });
  } catch (cause) {
    console.error("Restore failed:", cause);
    notify({
      tone: "error",
      title: "Не удалось восстановить бэкап",
    });
  } finally {
    restoring.value = false;
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

async function locateSavePath() {
  if (!game.value) return;
  locatingSavePath.value = true;
  try {
    const info = await backupApi.findGameSavePaths(game.value.name, game.value.id);
    if (info.save_path) {
      savePathDraft.value = info.save_path;
      notify({
        tone: "success",
        title: "Путь найден",
      });
    } else {
      notify({
        tone: "warning",
        title: "Сохранения не найдены",
        description: "Укажите путь вручную.",
      });
    }
  } catch (cause) {
    console.error("Failed to locate saves:", cause);
    notify({
      tone: "error",
      title: "Ошибка поиска сохранений",
    });
  } finally {
    locatingSavePath.value = false;
  }
}

async function openSavePath() {
  if (!game.value || !savePathDraft.value.trim()) return;
  await openPath(resolveSavePathTemplate(savePathDraft.value.trim(), game.value));
}

async function chooseSaveFolder() {
  const selected = await pickDirectoryPath({
    title: "Выбрать папку с сохранениями",
  });
  if (selected) {
    savePathDraft.value = selected;
  }
}

async function chooseSaveFile() {
  const selected = await pickFilePath({
    title: "Выбрать файл сохранения",
  });
  if (selected) {
    savePathDraft.value = selected;
  }
}

function insertGamePathToken() {
  const current = savePathDraft.value.trim();
  if (current.startsWith(GAME_PATH_TOKEN)) return;
  const needsSeparator =
    current.length > 0 && !current.startsWith("\\") && !current.startsWith("/");
  savePathDraft.value = `${GAME_PATH_TOKEN}${needsSeparator ? "\\" : ""}${current}`;
}

async function saveGamePath() {
  if (!game.value) return;
  savingPath.value = true;
  try {
    const normalizedPath = savePathDraft.value.trim();
    await gamesStore.updateGame(game.value.id, {
      save_path: normalizedPath === "" ? null : normalizedPath,
    });
    notify({
      tone: "success",
      title: "Путь сохранен",
    });
  } catch (cause) {
    console.error("Failed to save game path:", cause);
    notify({
      tone: "error",
      title: "Не удалось сохранить путь",
    });
  } finally {
    savingPath.value = false;
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

async function handleSaveEdit() {
  if (!game.value) return;
  savingEdit.value = true;
  try {
    await gamesStore.updateGame(game.value.id, {
      name: editForm.name,
      description: editForm.description,
      background_image: editForm.background_image,
      cover_image: editForm.cover_image,
    });
    await gamesStore.refreshGames();
    showEditDialog.value = false;
  } catch (cause) {
    console.error("Failed to update game:", cause);
  } finally {
    savingEdit.value = false;
  }
}

async function handleSearchGoogleImage(
  searchQueryText: string,
  target: "background" | "cover",
) {
  const suffix = target === "cover" ? " cover" : " background";
  const query = encodeURIComponent(`${searchQueryText}${suffix}`);
  await openPath(`https://www.google.com/search?tbm=isch&q=${query}`);
}

async function searchMetadata() {
  if (!metadataQuery.value.trim()) return;
  searchingMetadata.value = true;
  try {
    metadataResults.value = await metadataApi.search(metadataQuery.value.trim());
  } catch (cause) {
    console.error("Metadata search failed:", cause);
  } finally {
    searchingMetadata.value = false;
  }
}

async function applyMetadata(rawgGame: RawgGame) {
  if (!game.value) return;
  applyingMetadata.value = true;
  try {
    await metadataApi.apply(game.value.id, rawgGame.id, renameFromMetadata.value);
    await gamesStore.refreshGames();
    showMetadataSearch.value = false;
    metadataResults.value = [];
    metadataQuery.value = "";
  } catch (cause) {
    console.error("Failed to apply metadata:", cause);
  } finally {
    applyingMetadata.value = false;
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

    <section
      data-testid="game-detail-hero"
      class="overflow-hidden rounded-[28px] border border-border/70 bg-card/80 shadow-[0_24px_60px_rgba(0,0,0,0.24)]"
    >
      <div class="relative min-h-[360px] overflow-hidden">
        <img
          v-if="heroImage"
          :src="heroImage"
          :alt="game.name"
          class="absolute inset-0 h-full w-full object-cover"
        />
        <div
          v-else
          class="absolute inset-0 bg-[radial-gradient(circle_at_top_right,rgba(129,140,248,0.4),transparent_45%),linear-gradient(135deg,rgba(23,23,28,0.96),rgba(55,65,81,0.92))]"
        />
        <div class="absolute inset-0 bg-gradient-to-t from-black via-black/50 to-black/10" />
        <div class="relative flex min-h-[360px] flex-col justify-end p-6 sm:p-8">
          <div class="max-w-[480px] space-y-3">
            <h1 class="text-3xl font-semibold tracking-tight text-white sm:text-4xl">
              {{ game.name }}
            </h1>
            <div v-if="heroGenres" class="text-xs uppercase tracking-[0.22em] text-white/70">
              {{ heroGenres }}
            </div>
            <p
              v-if="heroDescription"
              data-testid="game-detail-hero-description"
              class="max-w-[440px] overflow-hidden text-sm leading-6 text-white/78 [display:-webkit-box] [-webkit-box-orient:vertical] [-webkit-line-clamp:2]"
            >
              {{ heroDescription }}
            </p>
            <div class="text-sm text-white/70">
              {{ heroMeta }}
            </div>
          </div>

          <div class="mt-6 flex flex-wrap items-end gap-3">
            <button
              type="button"
              class="inline-flex h-11 items-center gap-2 rounded-full bg-primary px-5 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="launching || restoring || isMissing"
              @click="void handleLaunch()"
            >
              <HardDrive v-if="isMissing" class="h-4 w-4" />
              <Activity v-else-if="runningCount > 0" class="h-4 w-4" />
              <Loader2
                v-else-if="launching || restoring || checkingRunning"
                class="h-4 w-4 animate-spin"
              />
              <Play v-else class="h-4 w-4" />
              {{
                isMissing
                  ? "Не установлена"
                  : runningCount > 0
                    ? "Закрыть игру"
                    : "Играть"
              }}
            </button>

            <button
              type="button"
              class="inline-flex h-11 items-center gap-2 rounded-full border border-white/15 bg-black/20 px-4 text-sm text-white transition-colors hover:bg-black/35"
              @click="void handleToggleFavorite()"
            >
              <Heart class="h-4 w-4" :class="{ 'fill-current text-rose-400': game.is_favorite }" />
              {{ game.is_favorite ? "В избранном" : "В избранное" }}
            </button>

            <button
              type="button"
              data-testid="game-detail-description-button"
              class="inline-flex h-11 items-center gap-2 rounded-full border border-white/15 bg-black/20 px-4 text-sm text-white transition-colors hover:bg-black/35"
              @click="showDescriptionModal = true"
            >
              Описание
            </button>
          </div>
        </div>
      </div>
    </section>

    <div class="grid gap-6 xl:grid-cols-[minmax(0,1.2fr)_minmax(0,0.8fr)]">
      <section class="space-y-6">
        <div class="rounded-2xl border border-border/70 bg-card/80 p-4 shadow-sm">
          <div class="mb-4 flex items-center justify-between gap-3">
            <div>
              <div class="text-base font-semibold">Состояние игры</div>
              <div class="text-xs text-muted-foreground">
                Запуск, статус прохождения и личные данные
              </div>
            </div>
            <select
              v-model="playStatus"
              class="h-10 rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
              @change="void savePlayStatus()"
            >
              <option value="not_started">Не начато</option>
              <option value="in_progress">В процессе</option>
              <option value="completed">Пройдено</option>
              <option value="abandoned">Брошено</option>
            </select>
          </div>

          <div class="grid gap-4 sm:grid-cols-2">
            <div class="rounded-xl border border-border/70 bg-background/40 p-4">
              <div class="text-xs text-muted-foreground">Личный рейтинг</div>
              <div class="mt-3 flex items-center gap-3">
                <input
                  v-model.number="userRating"
                  type="number"
                  min="1"
                  max="7"
                  class="h-11 w-24 rounded-xl border border-border/70 bg-card/70 px-3 text-lg font-semibold outline-none"
                />
                <button
                  type="button"
                  class="inline-flex h-11 items-center gap-2 rounded-xl bg-primary px-4 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
                  :disabled="savingRating"
                  @click="void saveUserRating()"
                >
                  <Loader2 v-if="savingRating" class="h-4 w-4 animate-spin" />
                  <Save v-else class="h-4 w-4" />
                  Сохранить
                </button>
                <button
                  type="button"
                  class="inline-flex h-11 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
                  @click="
                    ratingDraft = userRating ?? 4;
                    showRatingModal = true;
                  "
                >
                  <Star class="h-4 w-4" />
                  Быстрый ввод
                </button>
              </div>
            </div>

            <div class="rounded-xl border border-border/70 bg-background/40 p-4">
              <div class="text-xs text-muted-foreground">Время в игре</div>
              <div class="mt-3 flex items-center gap-2 text-2xl font-semibold">
                <Timer class="h-5 w-5" />
                {{ formatPlaytime(game.total_playtime) }}
              </div>
              <div class="mt-3">
                <span class="rounded-full border px-2 py-1 text-xs" :class="PLAY_STATUS_TONES[playStatus]">
                  {{ PLAY_STATUS_LABELS[playStatus] }}
                </span>
              </div>
            </div>
          </div>

          <div class="mt-4 rounded-xl border border-border/70 bg-background/40 p-4">
            <div class="mb-2 text-xs text-muted-foreground">Личная заметка</div>
            <textarea
              v-model="userNote"
              class="min-h-[160px] w-full rounded-xl border border-border/70 bg-card/70 px-4 py-3 text-sm outline-none"
              placeholder="Что стоит помнить об этой игре?"
            />
            <div class="mt-3 flex justify-end">
              <button
                type="button"
                class="inline-flex h-10 items-center gap-2 rounded-xl bg-primary px-4 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
                :disabled="savingNote"
                @click="void saveUserNote()"
              >
                <Loader2 v-if="savingNote" class="h-4 w-4 animate-spin" />
                <Save v-else class="h-4 w-4" />
                Сохранить заметку
              </button>
            </div>
          </div>
        </div>

        <div class="rounded-2xl border border-border/70 bg-card/80 p-4 shadow-sm">
          <div class="mb-4 flex items-center justify-between gap-3">
            <div>
              <div class="text-base font-semibold">Описание и детали</div>
              <div class="text-xs text-muted-foreground">
                Основные сведения о релизе и локальном пути запуска
              </div>
            </div>
            <button
              type="button"
              class="inline-flex h-10 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
              @click="showEditDialog = true"
            >
              <Pencil class="h-4 w-4" />
              Редактировать
            </button>
          </div>

          <div class="rounded-xl border border-border/70 bg-background/40 p-4">
            <div class="mb-3 text-xs text-muted-foreground">Описание</div>
            <p class="whitespace-pre-wrap text-sm leading-6 text-foreground/88">
              {{ game.description || "Описание отсутствует." }}
            </p>
          </div>

          <div class="mt-4 grid gap-3 sm:grid-cols-2">
            <div class="rounded-xl border border-border/70 bg-background/40 p-4">
              <div class="text-xs text-muted-foreground">Дата выхода</div>
              <div class="mt-2 text-sm font-medium">
                {{ game.released ? new Date(game.released).toLocaleDateString("ru-RU") : "—" }}
              </div>
            </div>
            <div class="rounded-xl border border-border/70 bg-background/40 p-4">
              <div class="text-xs text-muted-foreground">Платформы</div>
              <div class="mt-2 text-sm font-medium">{{ game.platforms || "—" }}</div>
            </div>
            <div class="rounded-xl border border-border/70 bg-background/40 p-4">
              <div class="text-xs text-muted-foreground">Разработчики</div>
              <div class="mt-2 text-sm font-medium">{{ game.developers || "—" }}</div>
            </div>
            <div class="rounded-xl border border-border/70 bg-background/40 p-4">
              <div class="text-xs text-muted-foreground">Издатели</div>
              <div class="mt-2 text-sm font-medium">{{ game.publishers || "—" }}</div>
            </div>
          </div>

          <div class="mt-4 rounded-xl border border-border/70 bg-background/40 p-4">
            <div class="mb-2 text-xs text-muted-foreground">Исполняемый файл</div>
            <div class="break-all font-mono text-xs text-foreground/88">
              {{ game.exe_path }}
            </div>
          </div>
        </div>

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
                  :checked="game.backup_enabled"
                  type="checkbox"
                  class="h-4 w-4 rounded border-border/70"
                  @change="void toggleBackupEnabled(($event.target as HTMLInputElement).checked)"
                />
                Автобэкап
              </label>
              <button
                type="button"
                class="inline-flex h-10 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60 disabled:cursor-not-allowed disabled:opacity-60"
                :disabled="creatingBackup"
                @click="void createManualBackup()"
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
              v-model="savePathDraft"
              class="h-11 w-full rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
              placeholder="Например: {PATHTOGAME}\\saves"
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
                @click="void chooseSaveFolder()"
              >
                <FolderOpen class="h-4 w-4" />
                Папка
              </button>
              <button
                type="button"
                class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70"
                @click="void chooseSaveFile()"
              >
                <FileIcon class="h-4 w-4" />
                Файл
              </button>
              <button
                type="button"
                class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70"
                @click="insertGamePathToken()"
              >
                <Star class="h-4 w-4" />
                {{ GAME_PATH_TOKEN }}
              </button>
              <button
                type="button"
                class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
                :disabled="locatingSavePath"
                @click="void locateSavePath()"
              >
                <Loader2 v-if="locatingSavePath" class="h-4 w-4 animate-spin" />
                <Search v-else class="h-4 w-4" />
                Найти
              </button>
              <button
                type="button"
                class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
                :disabled="!canOpenSavePath"
                @click="void openSavePath()"
              >
                <ExternalLink class="h-4 w-4" />
                Открыть
              </button>
              <button
                type="button"
                class="ml-auto inline-flex items-center gap-2 rounded-xl bg-primary px-4 py-2 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
                :disabled="savingPath"
                @click="void saveGamePath()"
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
                    @click="void restoreBackup(latestBackup.id)"
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
                      @click="void restoreBackup(backup.id)"
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
      </section>

      <section class="space-y-6">
        <div class="rounded-2xl border border-border/70 bg-card/80 p-4 shadow-sm">
          <div class="mb-4 flex items-center justify-between gap-3">
            <div>
              <div class="text-base font-semibold">Метаданные</div>
              <div class="text-xs text-muted-foreground">
                RAWG, изображения и ручное редактирование
              </div>
            </div>
            <div class="flex gap-2">
              <button
                type="button"
                class="inline-flex h-10 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
                @click="
                  metadataQuery = game.name;
                  showMetadataSearch = true;
                "
              >
                <Search class="h-4 w-4" />
                RAWG
              </button>
              <button
                type="button"
                class="inline-flex h-10 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
                @click="showEditDialog = true"
              >
                <Pencil class="h-4 w-4" />
                Редактировать
              </button>
            </div>
          </div>

          <div class="grid gap-3 sm:grid-cols-2">
            <div class="rounded-xl border border-border/70 bg-background/40 p-4">
              <div class="text-xs text-muted-foreground">RAWG ID</div>
              <div class="mt-2 text-sm font-medium">{{ game.rawg_id ?? "—" }}</div>
            </div>
            <div class="rounded-xl border border-border/70 bg-background/40 p-4">
              <div class="text-xs text-muted-foreground">Metacritic</div>
              <div class="mt-2 text-sm font-medium">{{ game.metacritic ?? "—" }}</div>
            </div>
            <div class="rounded-xl border border-border/70 bg-background/40 p-4">
              <div class="text-xs text-muted-foreground">Фон</div>
              <div class="mt-2 text-sm font-medium">
                {{ game.background_image ? "Есть" : "Нет" }}
              </div>
            </div>
            <div class="rounded-xl border border-border/70 bg-background/40 p-4">
              <div class="text-xs text-muted-foreground">Обложка</div>
              <div class="mt-2 text-sm font-medium">
                {{ game.cover_image ? "Есть" : "Нет" }}
              </div>
            </div>
          </div>
        </div>

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

    <Teleport to="body">
      <div
        v-if="showDescriptionModal && heroDescription"
        class="fixed inset-0 z-[120] flex items-center justify-center bg-black/78 p-4 backdrop-blur-sm"
        @mousedown.self="showDescriptionModal = false"
      >
        <div
          data-testid="game-detail-description-modal"
          class="w-full max-w-2xl overflow-hidden rounded-2xl border border-border/60 bg-card/92 shadow-[0_30px_80px_rgba(8,12,24,0.55)]"
        >
          <div class="flex items-center justify-between border-b border-border/60 px-5 py-4">
            <h2 class="text-lg font-semibold">Описание игры</h2>
            <button
              type="button"
              class="inline-flex h-9 w-9 items-center justify-center rounded-xl border border-border/70 bg-card/80 transition-colors hover:bg-accent/70"
              @click="showDescriptionModal = false"
            >
              <X class="h-4 w-4" />
            </button>
          </div>
          <div class="max-h-[70vh] overflow-auto px-5 py-4">
            <p class="whitespace-pre-wrap text-sm leading-7 text-foreground/88 sm:text-[15px]">
              {{ heroDescription }}
            </p>
          </div>
        </div>
      </div>
    </Teleport>

    <Teleport to="body">
      <div
        v-if="showRatingModal"
        class="fixed inset-0 z-[121] flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm"
        @mousedown.self="showRatingModal = false"
      >
        <div class="w-full max-w-sm rounded-2xl border border-border/60 bg-card/92 p-6 shadow-[0_30px_80px_rgba(8,12,24,0.55)]">
          <div class="mb-6 flex items-center justify-between">
            <div class="flex h-12 w-12 items-center justify-center rounded-full border border-white/10 bg-white/8 text-2xl font-bold text-white shadow-[0_10px_24px_rgba(0,0,0,0.22)]">
              {{ ratingDraft }}
            </div>
            <button
              type="button"
              class="inline-flex h-10 w-10 items-center justify-center rounded-xl border border-border/70 bg-background/50 transition-colors hover:bg-accent/60"
              @click="showRatingModal = false"
            >
              <X class="h-4 w-4" />
            </button>
          </div>

          <input
            v-model.number="ratingDraft"
            type="number"
            min="1"
            max="7"
            step="1"
            class="w-full rounded-2xl border border-border/60 bg-background/20 py-6 text-center text-6xl font-bold outline-none"
          />

          <div class="mt-6 flex justify-end gap-2">
            <button
              type="button"
              class="inline-flex h-10 items-center gap-2 rounded-xl border border-border/70 bg-background/50 px-4 text-sm transition-colors hover:bg-accent/60"
              @click="showRatingModal = false"
            >
              Отмена
            </button>
            <button
              type="button"
              class="inline-flex h-10 items-center gap-2 rounded-xl bg-primary px-4 text-sm font-medium text-primary-foreground transition-colors hover:bg-accent hover:text-white"
              @click="
                userRating = Math.max(1, Math.min(7, Number(ratingDraft) || 1));
                showRatingModal = false;
                void saveUserRating();
              "
            >
              <Save class="h-4 w-4" />
              Сохранить
            </button>
          </div>
        </div>
      </div>
    </Teleport>

    <Teleport to="body">
      <div
        v-if="showEditDialog"
        class="fixed inset-0 z-[122] flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm"
        @mousedown.self="showEditDialog = false"
      >
        <div class="flex max-h-[90vh] w-full max-w-2xl flex-col overflow-hidden rounded-xl border bg-card shadow-2xl">
          <div class="flex items-center justify-between border-b p-6">
            <h2 class="text-xl font-bold">Редактировать игру</h2>
            <div class="flex items-center gap-2">
              <button
                type="button"
                class="inline-flex items-center gap-2 rounded-xl bg-primary px-4 py-2 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white"
                :disabled="savingEdit"
                @click="void handleSaveEdit()"
              >
                <Loader2 v-if="savingEdit" class="h-4 w-4 animate-spin" />
                <Save v-else class="h-4 w-4" />
                Сохранить
              </button>
              <button
                type="button"
                class="inline-flex h-10 w-10 items-center justify-center rounded-xl border border-border/70 bg-card/80 transition-colors hover:bg-accent/70"
                @click="showEditDialog = false"
              >
                <X class="h-5 w-5" />
              </button>
            </div>
          </div>

          <div class="flex-1 overflow-auto p-6">
            <div class="space-y-6">
              <div class="space-y-2">
                <label class="text-sm font-medium" for="edit-name">Название</label>
                <input
                  id="edit-name"
                  v-model="editForm.name"
                  class="h-11 w-full rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
                />
              </div>

              <div class="space-y-2">
                <label class="text-sm font-medium" for="edit-description">Описание</label>
                <textarea
                  id="edit-description"
                  v-model="editForm.description"
                  class="min-h-[120px] w-full rounded-xl border border-border/70 bg-card/70 px-4 py-3 text-sm outline-none"
                />
              </div>

              <div class="space-y-5">
                <div class="space-y-2">
                  <label class="block text-sm font-medium" for="edit-background-url">
                    URL фона
                  </label>
                  <div class="flex gap-2">
                    <input
                      id="edit-background-url"
                      v-model="editForm.background_image"
                      placeholder="https://..."
                      class="h-11 flex-1 rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
                    />
                    <button
                      type="button"
                      class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-4 py-2 text-sm transition-colors hover:bg-accent/70"
                      @click="void handleSearchGoogleImage(editForm.name || game.name, 'background')"
                    >
                      <Globe class="h-4 w-4" />
                      Google
                    </button>
                  </div>
                  <div class="aspect-video overflow-hidden rounded-xl border border-border/70 bg-background/40">
                    <img
                      v-if="editForm.background_image"
                      :src="editForm.background_image"
                      :alt="`${editForm.name || game.name} background`"
                      class="h-full w-full object-cover"
                    />
                    <div v-else class="flex h-full items-center justify-center text-muted-foreground">
                      <ImageIcon class="mr-2 h-8 w-8 opacity-50" />
                      Нет изображения
                    </div>
                  </div>
                </div>

                <div class="space-y-2">
                  <label class="block text-sm font-medium" for="edit-cover-url">
                    URL обложки
                  </label>
                  <div class="flex gap-2">
                    <input
                      id="edit-cover-url"
                      v-model="editForm.cover_image"
                      placeholder="https://..."
                      class="h-11 flex-1 rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
                    />
                    <button
                      type="button"
                      class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-4 py-2 text-sm transition-colors hover:bg-accent/70"
                      @click="void handleSearchGoogleImage(editForm.name || game.name, 'cover')"
                    >
                      <Globe class="h-4 w-4" />
                      Google
                    </button>
                  </div>
                  <div class="aspect-[2/3] w-40 overflow-hidden rounded-xl border border-border/70 bg-background/40">
                    <img
                      v-if="editForm.cover_image"
                      :src="editForm.cover_image"
                      :alt="`${editForm.name || game.name} cover`"
                      class="h-full w-full object-cover"
                    />
                    <div v-else class="flex h-full items-center justify-center px-3 text-center text-xs text-muted-foreground">
                      Нет обложки
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Teleport>

    <Teleport to="body">
      <div
        v-if="showMetadataSearch"
        class="fixed inset-0 z-[123] flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm"
        @mousedown.self="showMetadataSearch = false"
      >
        <div class="flex max-h-[80vh] w-full max-w-lg flex-col rounded-lg bg-card">
          <div class="border-b p-4">
            <h2 class="text-lg font-semibold">Поиск метаданных</h2>
            <p class="text-sm text-muted-foreground">Поиск информации об игре в базе RAWG</p>
          </div>

          <div class="flex flex-1 flex-col overflow-hidden p-4">
            <div class="mb-4 flex gap-2">
              <input
                v-model="metadataQuery"
                placeholder="Название игры..."
                class="h-10 flex-1 rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
                @keydown.enter="void searchMetadata()"
              />
              <button
                type="button"
                class="inline-flex h-10 w-10 items-center justify-center rounded-xl bg-primary text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
                :disabled="searchingMetadata"
                @click="void searchMetadata()"
              >
                <Loader2 v-if="searchingMetadata" class="h-4 w-4 animate-spin" />
                <Search v-else class="h-4 w-4" />
              </button>
            </div>

            <label class="mb-3 flex items-center justify-between gap-3 text-sm text-muted-foreground">
              <span>Использовать название из RAWG</span>
              <input
                v-model="renameFromMetadata"
                type="checkbox"
                class="h-4 w-4 rounded border-border/70"
              />
            </label>

            <div class="flex-1 overflow-auto">
              <div v-if="metadataResults.length > 0" class="space-y-2">
                <button
                  v-for="result in metadataResults"
                  :key="result.id"
                  type="button"
                  class="flex w-full items-center gap-3 rounded-md border border-transparent p-3 text-left transition-colors hover:border-border hover:bg-secondary/70"
                  @click="void applyMetadata(result)"
                >
                  <img
                    v-if="result.background_image"
                    :src="result.background_image"
                    :alt="result.name"
                    class="h-16 w-16 rounded object-cover"
                  />
                  <div
                    v-else
                    class="flex h-16 w-16 items-center justify-center rounded bg-muted"
                  >
                    <Gamepad2 class="h-6 w-6 text-muted-foreground" />
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="truncate font-medium">{{ result.name }}</div>
                    <div class="text-sm text-muted-foreground">
                      {{ result.released?.slice(0, 4) || "?" }}
                      <span v-if="result.metacritic"> • {{ result.metacritic }}</span>
                    </div>
                  </div>
                  <Loader2 v-if="applyingMetadata" class="h-4 w-4 animate-spin" />
                  <ExternalLink v-else class="h-4 w-4 text-muted-foreground" />
                </button>
              </div>
              <div v-else class="py-8 text-center text-muted-foreground">
                Введите название для поиска
              </div>
            </div>
          </div>

          <div class="flex justify-end border-t p-4">
            <button
              type="button"
              class="inline-flex h-10 items-center rounded-xl border border-border/70 bg-card/80 px-4 text-sm transition-colors hover:bg-accent/70"
              @click="showMetadataSearch = false"
            >
              Закрыть
            </button>
          </div>
        </div>
      </div>
    </Teleport>

    <div v-if="backupProgress.active" class="fixed right-4 bottom-4 left-4 z-[124] lg:left-[300px]">
      <div class="flex items-center gap-3 rounded-xl border border-border/60 bg-card/80 px-4 py-3 shadow-[0_16px_40px_rgba(8,12,24,0.45)] backdrop-blur-xl">
        <Loader2 class="h-4 w-4 animate-spin text-primary" />
        <div class="min-w-0">
          <div class="text-sm font-medium">
            {{
              backupProgress.stage === "scan"
                ? "Подготовка бэкапа"
                : backupProgress.stage === "copy"
                  ? "Создание бэкапа"
                  : backupProgress.stage === "restore"
                    ? "Восстановление бэкапа"
                    : "Обработка"
            }}
          </div>
          <div class="truncate text-xs text-muted-foreground">
            {{ backupProgress.message }}
          </div>
        </div>
        <div v-if="backupProgress.total > 0" class="ml-auto text-xs tabular-nums text-muted-foreground">
          {{ backupProgress.done }}/{{ backupProgress.total }}
        </div>
      </div>
    </div>
  </div>
</template>
