<script setup lang="ts">
// CataloguePage — RAWG search + apply metadata к существующему game_obj.
//
// Поток: query → debounced 500ms → arrancador.rawg.search → cards →
// «Применить к…» → выбор game_obj из библиотеки → arrancador.rawg.apply.

import { onMounted, onBeforeUnmount, ref, watch } from "vue";
import { Search } from "lucide-vue-next";
import { useRouter } from "vue-router";

import { EmptyState, Dropdown, Modal } from "@kepler/visuals";

import { useGames } from "../composables/useGames";
import {
  requireArrancadorApi,
  type RawgGame,
} from "../lib/arrancadorApi";

const { games } = useGames();
const router = useRouter();

const rawgKey = ref<string | null>(null);
const rawgKeyChecked = ref(false);

async function loadRawgKey() {
  try {
    const api = requireArrancadorApi();
    const res = await api.config.getRawgKey();
    rawgKey.value = res.key ?? null;
  } catch {
    rawgKey.value = null;
  } finally {
    rawgKeyChecked.value = true;
  }
}

onMounted(loadRawgKey);

const query = ref("");
const debouncedQuery = ref("");
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

watch(query, (next) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    debouncedQuery.value = next.trim();
  }, 500);
});

onBeforeUnmount(() => {
  if (debounceTimer) clearTimeout(debounceTimer);
});

const results = ref<RawgGame[]>([]);
const searching = ref(false);
const searchError = ref<string | null>(null);

watch(debouncedQuery, async (q) => {
  if (!q) {
    results.value = [];
    searchError.value = null;
    return;
  }
  searching.value = true;
  searchError.value = null;
  try {
    const api = requireArrancadorApi();
    const res = await api.rawg.search(q);
    results.value = res.results ?? [];
  } catch (cause) {
    results.value = [];
    searchError.value =
      cause instanceof Error ? cause.message : "Ошибка поиска RAWG";
  } finally {
    searching.value = false;
  }
});

// Apply modal state
const applyModalOpen = ref(false);
const applyTarget = ref<RawgGame | null>(null);
const selectedGameId = ref<string | null>(null);
const applying = ref(false);
const applyError = ref<string | null>(null);
const applySuccess = ref<string | null>(null);

function openApplyModal(rawg: RawgGame) {
  applyTarget.value = rawg;
  selectedGameId.value = null;
  applyError.value = null;
  applySuccess.value = null;
  applyModalOpen.value = true;
}

function closeApplyModal() {
  applyModalOpen.value = false;
}

const gameOptions = ref<{ value: string; label: string }[]>([]);
watch(
  games,
  (list) => {
    gameOptions.value = list.map((g) => ({ value: g.id, label: g.name }));
  },
  { immediate: true, deep: true },
);

async function onConfirmApply() {
  if (!applyTarget.value || !selectedGameId.value) return;
  applying.value = true;
  applyError.value = null;
  applySuccess.value = null;
  try {
    const api = requireArrancadorApi();
    const res = await api.rawg.apply(selectedGameId.value, applyTarget.value.id);
    if (res.ok) {
      applySuccess.value = "Metadata применены.";
      setTimeout(() => closeApplyModal(), 1000);
    } else {
      applyError.value = res.error ?? "Не удалось применить metadata";
    }
  } catch (cause) {
    applyError.value =
      cause instanceof Error ? cause.message : "Ошибка применения metadata";
  } finally {
    applying.value = false;
  }
}

function goToSettings() {
  router.push("/settings");
}

function releaseYear(released: string | null | undefined): string {
  if (!released) return "—";
  return released.slice(0, 4);
}

function genresLine(rawg: RawgGame): string {
  if (!rawg.genres || rawg.genres.length === 0) return "";
  return rawg.genres.map((g) => g.name).join(", ");
}
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">Каталог</h1>

    <EmptyState
      v-if="rawgKeyChecked && !rawgKey"
      title="RAWG API ключ не задан"
      description="Чтобы искать игры в каталоге RAWG, укажите ключ в настройках. Получить ключ можно бесплатно на rawg.io/apidocs."
    >
      <template #action>
        <button
          type="button"
          class="arrancador-catalogue-action"
          @click="goToSettings"
        >
          Перейти в настройки
        </button>
      </template>
    </EmptyState>

    <template v-else>
      <div class="arrancador-catalogue-search">
        <Search :size="16" class="arrancador-catalogue-search__icon" />
        <input
          v-model="query"
          type="text"
          placeholder="Поиск в RAWG…"
          class="arrancador-catalogue-search__input"
        />
      </div>

      <div v-if="searchError" class="arrancador-error">{{ searchError }}</div>

      <EmptyState
        v-if="!searching && !query && results.length === 0"
        title="Введите название игры"
        description="Поиск работает через публичный API RAWG."
      />

      <EmptyState
        v-else-if="!searching && query && results.length === 0 && !searchError"
        title="Ничего не найдено"
        description="Попробуйте другое название."
      />

      <div v-if="searching" class="arrancador-catalogue-loading">Поиск…</div>

      <div v-if="results.length > 0" class="arrancador-catalogue-grid">
        <article
          v-for="rawg in results"
          :key="rawg.id"
          class="arrancador-catalogue-card"
        >
          <div
            class="arrancador-catalogue-card__cover"
            :style="
              rawg.background_image
                ? { backgroundImage: `url(${rawg.background_image})` }
                : {}
            "
          >
            <span v-if="!rawg.background_image" aria-hidden="true">🎮</span>
          </div>
          <div class="arrancador-catalogue-card__body">
            <div class="arrancador-catalogue-card__title">{{ rawg.name }}</div>
            <div class="arrancador-catalogue-card__meta">
              <span>{{ releaseYear(rawg.released) }}</span>
              <span v-if="genresLine(rawg)"> · {{ genresLine(rawg) }}</span>
            </div>
            <button
              type="button"
              class="arrancador-catalogue-action"
              @click="openApplyModal(rawg)"
            >
              Применить metadata к…
            </button>
          </div>
        </article>
      </div>
    </template>

    <Modal
      :open="applyModalOpen"
      title="Применить metadata RAWG"
      @close="closeApplyModal"
    >
      <div v-if="applyTarget" class="arrancador-catalogue-apply">
        <p class="arrancador-catalogue-apply__source">
          Источник: <strong>{{ applyTarget.name }}</strong>
        </p>
        <p class="arrancador-catalogue-apply__hint">
          Выберите игру в библиотеке, к которой нужно применить metadata
          (название, обложка, жанры, описание).
        </p>
        <Dropdown
          v-model="selectedGameId"
          :options="gameOptions"
          placeholder="Игра из библиотеки…"
        />
        <div v-if="applyError" class="arrancador-error">{{ applyError }}</div>
        <div v-if="applySuccess" class="arrancador-catalogue-apply__ok">
          {{ applySuccess }}
        </div>
      </div>
      <template #footer>
        <button
          type="button"
          class="arrancador-catalogue-action"
          :disabled="applying"
          @click="closeApplyModal"
        >
          Отмена
        </button>
        <button
          type="button"
          class="arrancador-catalogue-action arrancador-catalogue-action--primary"
          :disabled="!selectedGameId || applying"
          @click="onConfirmApply"
        >
          {{ applying ? "Применяю…" : "Применить" }}
        </button>
      </template>
    </Modal>
  </section>
</template>

<style scoped>
.arrancador-catalogue-search {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 16px;
  padding: 0 12px;
  height: 38px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--background);
}

.arrancador-catalogue-search__icon {
  color: var(--muted-foreground);
}

.arrancador-catalogue-search__input {
  flex: 1;
  border: 0;
  background: transparent;
  outline: none;
  color: var(--foreground);
}

.arrancador-catalogue-loading {
  font-size: 13px;
  color: var(--muted-foreground);
  padding: 8px 0;
}

.arrancador-catalogue-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 16px;
}

.arrancador-catalogue-card {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: var(--card);
  overflow: hidden;
}

.arrancador-catalogue-card__cover {
  height: 120px;
  background-color: var(--muted);
  background-position: center;
  background-size: cover;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  color: var(--muted-foreground);
}

.arrancador-catalogue-card__body {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
}

.arrancador-catalogue-card__title {
  font-size: 13px;
  font-weight: 600;
  color: var(--foreground);
}

.arrancador-catalogue-card__meta {
  font-size: 11px;
  color: var(--muted-foreground);
}

.arrancador-catalogue-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 28px;
  padding: 0 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--card);
  color: var(--foreground);
  font-size: 12px;
  transition: background 120ms var(--easing-standard);
}

.arrancador-catalogue-action:not(:disabled):hover {
  background: var(--muted);
}

.arrancador-catalogue-action:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.arrancador-catalogue-action--primary {
  background: var(--accent, var(--card));
  color: var(--accent-foreground, var(--foreground));
  font-weight: 600;
}

.arrancador-catalogue-apply {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.arrancador-catalogue-apply__source {
  margin: 0;
  font-size: 13px;
}

.arrancador-catalogue-apply__hint {
  margin: 0;
  font-size: 12px;
  color: var(--muted-foreground);
}

.arrancador-catalogue-apply__ok {
  font-size: 12px;
  color: var(--accent, var(--foreground));
}
</style>
