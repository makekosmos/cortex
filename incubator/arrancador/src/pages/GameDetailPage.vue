<script setup lang="ts">
// GameDetailPage — game hub: запуск, metadata, SQOBA backup + ARK projection.

import { computed, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { Archive, Play, RefreshCw, Search } from "@lucide/vue";

import { EmptyState, Modal } from "@kosmos/visuals";

import { useGames } from "../composables/useGames";
import { requireArrancadorApi, type RawgGame } from "../lib/arrancadorApi";

const route = useRoute();
const router = useRouter();
const { games, loading, error, refresh } = useGames();

const gameId = computed(() => (typeof route.params.id === "string" ? route.params.id : ""));

const game = computed(() => games.value.find((g) => g.id === gameId.value) ?? null);

const heroImage = computed(() => game.value?.backgroundImage ?? game.value?.coverImage ?? null);

const genres = computed(() => {
  const raw = game.value?.genres;
  if (!raw) return null;
  return raw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
    .slice(0, 3)
    .join(" · ");
});

const releaseYear = computed(() => {
  const released = game.value?.released;
  if (!released) return null;
  const year = new Date(released).getFullYear();
  return Number.isFinite(year) ? String(year) : null;
});

const playtimeLabel = computed(() => {
  const seconds = game.value?.totalPlaytime ?? 0;
  if (seconds <= 0) return "Не запускалась";
  const hours = seconds / 3600;
  return hours < 1 ? `${Math.round(seconds / 60)} мин` : `${hours.toFixed(1)} ч`;
});

function goBack() {
  router.back();
}

const actionBusy = ref<"launch" | "backup" | "metadata" | null>(null);
const actionMessage = ref<string | null>(null);
const actionIsError = ref(false);
const metadataModalOpen = ref(false);
const metadataQuery = ref("");
const metadataResults = ref<RawgGame[]>([]);
const metadataSearching = ref(false);
const metadataApplyingId = ref<number | null>(null);
const metadataError = ref<string | null>(null);

function setActionMessage(message: string, isError = false) {
  actionMessage.value = message;
  actionIsError.value = isError;
}

async function runAction(name: "launch" | "backup" | "metadata", fn: () => Promise<string>) {
  if (actionBusy.value || !game.value) return;
  actionBusy.value = name;
  actionMessage.value = null;
  actionIsError.value = false;
  try {
    setActionMessage(await fn());
    await refresh();
  } catch (cause) {
    setActionMessage(cause instanceof Error ? cause.message : "Действие не выполнено", true);
  } finally {
    actionBusy.value = null;
  }
}

function launchGame() {
  void runAction("launch", async () => {
    const current = game.value;
    if (!current) throw new Error("Игра не найдена");
    const result = await requireArrancadorApi().launch(current.id);
    if (!result.ok) throw new Error(result.error || "Не удалось запустить");
    return `Запущено (PID ${result.pid})`;
  });
}

function backupSaves() {
  void runAction("backup", async () => {
    const current = game.value;
    if (!current) throw new Error("Игра не найдена");
    const result = await requireArrancadorApi().sqoba.backup(current.id);
    return `Бэкап создан: ${result.files_count} файл(ов)`;
  });
}

function openMetadataModal() {
  const current = game.value;
  if (!current || actionBusy.value) return;
  metadataQuery.value = current.name;
  metadataResults.value = [];
  metadataError.value = null;
  metadataModalOpen.value = true;
  void searchMetadata();
}

function closeMetadataModal() {
  if (metadataSearching.value || metadataApplyingId.value !== null) return;
  metadataModalOpen.value = false;
}

async function searchMetadata() {
  const query = metadataQuery.value.trim();
  if (!query || metadataSearching.value) return;
  metadataSearching.value = true;
  metadataError.value = null;
  try {
    const found = await requireArrancadorApi().rawg.search(query);
    metadataResults.value = found.results;
    if (found.results.length === 0) metadataError.value = "RAWG ничего не нашёл по названию";
  } catch (cause) {
    metadataResults.value = [];
    metadataError.value = cause instanceof Error ? cause.message : "Не удалось найти metadata";
  } finally {
    metadataSearching.value = false;
  }
}

async function applyMetadata(result: RawgGame) {
  const current = game.value;
  if (!current || metadataApplyingId.value !== null) return;
  metadataApplyingId.value = result.id;
  metadataError.value = null;
  actionBusy.value = "metadata";
  actionMessage.value = null;
  actionIsError.value = false;
  try {
    const applied = await requireArrancadorApi().rawg.apply(current.id, result.id);
    if (!applied.ok) throw new Error(applied.error ?? "Не удалось применить metadata");
    metadataModalOpen.value = false;
    setActionMessage(`Metadata обновлены: ${result.name}`);
    await refresh();
  } catch (cause) {
    metadataError.value = cause instanceof Error ? cause.message : "Не удалось применить metadata";
  } finally {
    metadataApplyingId.value = null;
    actionBusy.value = null;
  }
}

function rawgMeta(result: RawgGame): string {
  const releaseYearValue = result.released ? new Date(result.released).getFullYear() : null;
  const parts = [
    Number.isFinite(releaseYearValue) ? String(releaseYearValue) : null,
    result.platforms
      ?.map((p) => p.platform.name)
      .filter(Boolean)
      .slice(0, 3)
      .join(", "),
  ].filter(Boolean);
  return parts.join(" · ");
}
</script>

<template>
  <section class="arrancador-page">
    <button type="button" class="arrancador-back" @click="goBack">← Назад</button>

    <div v-if="error" class="arrancador-error">{{ error }}</div>

    <EmptyState v-if="loading && games.length === 0" title="Загрузка…" />

    <EmptyState
      v-else-if="!game"
      title="Игра не найдена"
      description="Возможно, она была удалена из ARK. Вернитесь в Библиотеку."
    />

    <article v-else class="arrancador-detail">
      <div
        class="arrancador-detail__hero"
        :style="heroImage ? { backgroundImage: `url(${heroImage})` } : undefined"
      >
        <div class="arrancador-detail__hero-overlay">
          <h1 class="arrancador-detail__title">{{ game.name }}</h1>
          <div v-if="genres" class="arrancador-detail__genres">
            {{ genres }}
          </div>
        </div>
      </div>

      <div class="arrancador-detail__actions">
        <button
          type="button"
          class="arrancador-detail__action"
          :disabled="actionBusy !== null || (!game.exePath && game.source !== 'steam')"
          @click="launchGame"
        >
          <Play :size="14" />
          <span>{{ actionBusy === "launch" ? "Запускаю…" : "Запустить" }}</span>
        </button>
        <button
          type="button"
          class="arrancador-detail__action"
          :disabled="actionBusy !== null"
          @click="openMetadataModal"
        >
          <RefreshCw :size="14" />
          <span>{{ actionBusy === "metadata" ? "Обновляю…" : "Metadata по названию" }}</span>
        </button>
        <button
          type="button"
          class="arrancador-detail__action"
          :disabled="actionBusy !== null"
          @click="backupSaves"
        >
          <Archive :size="14" />
          <span>{{ actionBusy === "backup" ? "Создаю…" : "Бэкап сейвов" }}</span>
        </button>
      </div>

      <div
        v-if="actionMessage"
        class="arrancador-detail__action-message"
        :class="{ 'arrancador-detail__action-message--error': actionIsError }"
      >
        {{ actionMessage }}
      </div>

      <div class="arrancador-detail__meta">
        <div v-if="releaseYear" class="arrancador-detail__meta-row">
          <span class="arrancador-detail__meta-label">Год выпуска</span>
          <span class="arrancador-detail__meta-value">{{ releaseYear }}</span>
        </div>
        <div class="arrancador-detail__meta-row">
          <span class="arrancador-detail__meta-label">Время</span>
          <span class="arrancador-detail__meta-value">{{ playtimeLabel }}</span>
        </div>
        <div v-if="game.userRating" class="arrancador-detail__meta-row">
          <span class="arrancador-detail__meta-label">Оценка</span>
          <span class="arrancador-detail__meta-value"> {{ game.userRating.toFixed(1) }} / 5 </span>
        </div>
        <div v-if="game.playStatus" class="arrancador-detail__meta-row">
          <span class="arrancador-detail__meta-label">Статус</span>
          <span class="arrancador-detail__meta-value">
            {{ game.playStatus }}
          </span>
        </div>
        <div v-if="game.exePath" class="arrancador-detail__meta-row">
          <span class="arrancador-detail__meta-label">Путь</span>
          <span class="arrancador-detail__meta-value arrancador-detail__path">
            {{ game.exePath }}
          </span>
        </div>
        <div v-if="game.savePath" class="arrancador-detail__meta-row">
          <span class="arrancador-detail__meta-label">Сейвы</span>
          <span class="arrancador-detail__meta-value arrancador-detail__path">
            {{ game.savePath }}
          </span>
        </div>
      </div>

      <p v-if="game.description" class="arrancador-detail__description">
        {{ game.description }}
      </p>

      <p v-if="game.userNote" class="arrancador-detail__note">
        <strong>Заметка:</strong> {{ game.userNote }}
      </p>
    </article>

    <Modal
      :open="metadataModalOpen"
      title="Подтянуть metadata"
      width="min(720px, 94vw)"
      @close="closeMetadataModal"
    >
      <div class="arrancador-metadata-modal">
        <form class="arrancador-metadata-modal__search" @submit.prevent="searchMetadata">
          <input
            v-model="metadataQuery"
            class="arrancador-metadata-modal__input"
            type="text"
            placeholder="Название игры"
            :disabled="metadataSearching || metadataApplyingId !== null"
          />
          <button
            type="submit"
            class="arrancador-metadata-modal__button"
            :disabled="metadataSearching || metadataApplyingId !== null || !metadataQuery.trim()"
          >
            <Search :size="14" />
            <span>{{ metadataSearching ? "Ищу…" : "Найти" }}</span>
          </button>
        </form>

        <div
          v-if="metadataError"
          class="arrancador-metadata-modal__message arrancador-metadata-modal__message--error"
        >
          {{ metadataError }}
        </div>

        <div v-else-if="metadataSearching" class="arrancador-metadata-modal__message">
          Ищу совпадения в RAWG…
        </div>

        <div v-if="metadataResults.length > 0" class="arrancador-metadata-modal__results">
          <button
            v-for="result in metadataResults"
            :key="result.id"
            type="button"
            class="arrancador-metadata-modal__result"
            :disabled="metadataApplyingId !== null"
            @click="applyMetadata(result)"
          >
            <img
              v-if="result.background_image"
              class="arrancador-metadata-modal__cover"
              :src="result.background_image"
              alt=""
            />
            <span v-else class="arrancador-metadata-modal__cover-placeholder">RAWG</span>
            <span class="arrancador-metadata-modal__body">
              <span class="arrancador-metadata-modal__name">{{ result.name }}</span>
              <span v-if="rawgMeta(result)" class="arrancador-metadata-modal__meta">
                {{ rawgMeta(result) }}
              </span>
            </span>
            <span class="arrancador-metadata-modal__apply">
              {{ metadataApplyingId === result.id ? "Применяю…" : "Выбрать" }}
            </span>
          </button>
        </div>
      </div>
    </Modal>
  </section>
</template>
