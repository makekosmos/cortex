<script lang="ts">
// Публичные типы LayoutPage'а — переиспользуются GameCard / AppSidebar.
//
// `ArrancadorGame` — это denormalized projection game_obj.propsJson, не полный
// legacy `Game` (`apps/arrancador/src/types/index.ts`). Extension не пишет
// игр обратно в ARK, ему достаточно read-only поля для отрисовки.

export type ArrancadorSection =
  | "library"
  | "catalogue"
  | "statistics"
  | "settings";

export interface ArrancadorGame {
  id: string;
  name: string;
  coverImage: string | null;
  backgroundImage: string | null;
  genres: string | null;
  isFavorite: boolean;
}
</script>

<script setup lang="ts">
// Главная страница Arrancador-extension'а: titlebar + sidebar + content.
//
// Адаптация vs `apps/arrancador/src-vue/pages/LayoutPage.vue`:
//   - убраны Pinia store / vue-router / Teleport mobile menu / toast bridge.
//   - данные тянутся напрямую через window.kepler.ark.request(
//     "list_objects_by_type", { type_id: "game_obj" }), без electronAPI.
//   - Подписка на "entity_changed" для авто-refresh при изменении ARK
//     (legacy main process тоже эмитит этот event при upsert game_obj).
//   - Native game scanner и game launch остаются в `apps/arrancador/` legacy
//     (Phase 5). Здесь — только rendering того, что уже в ARK.

import { computed, onBeforeUnmount, onMounted, ref, shallowRef } from "vue";

import AppSidebar from "../components/AppSidebar.vue";
import AppSpotlight from "../components/AppSpotlight.vue";
import AppTitlebar from "../components/AppTitlebar.vue";
import GameCard from "../components/GameCard.vue";

// ---------------------------------------------------------------------------
// kepler.ark bridge — extension renderer ARK client.
// Schema: `apps/kepler-shell/electron/extension-preload.ts`.
// ---------------------------------------------------------------------------

interface KeplerArkBridge {
  request: (
    operation: string,
    params?: Record<string, unknown>,
  ) => Promise<unknown>;
  subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
}

interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string | null;
  contentJson: unknown;
  propsJson: unknown;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
}

function arkBridge(): KeplerArkBridge | null {
  const kepler = (
    window as unknown as { kepler?: { ark?: KeplerArkBridge } }
  ).kepler;
  return kepler?.ark ?? null;
}

function readString(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function readBoolean(value: unknown): boolean {
  return value === true;
}

function projectGame(record: ArkObjectRecord): ArrancadorGame {
  const props =
    record.propsJson && typeof record.propsJson === "object"
      ? (record.propsJson as Record<string, unknown>)
      : {};
  return {
    id: record.id,
    name: record.title ?? readString(props.name) ?? "Без названия",
    coverImage: readString(props.cover_image),
    backgroundImage: readString(props.background_image),
    genres: readString(props.genres),
    isFavorite: readBoolean(props.is_favorite),
  };
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const section = ref<ArrancadorSection>("library");
const sidebarHidden = ref(false);
const search = ref("");
const games = shallowRef<ArrancadorGame[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);

let unsubscribe: (() => void) | null = null;

async function refreshGames() {
  const bridge = arkBridge();
  if (!bridge) {
    error.value = "kepler.ark bridge недоступен";
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const result = await bridge.request("list_objects_by_type", {
      type_id: "game_obj",
    });
    const records = Array.isArray(result) ? (result as ArkObjectRecord[]) : [];
    games.value = records
      .filter((r) => r.deletedAt === null)
      .map(projectGame)
      .sort((a, b) => a.name.localeCompare(b.name, "ru"));
  } catch (cause) {
    // eslint-disable-next-line no-console
    console.error("[arrancador-extension] list_objects_by_type failed:", cause);
    error.value =
      cause instanceof Error ? cause.message : "Не удалось загрузить игры";
  } finally {
    loading.value = false;
  }
}

const filteredGames = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return games.value;
  return games.value.filter((g) => g.name.toLowerCase().includes(q));
});

onMounted(() => {
  void refreshGames();
  const bridge = arkBridge();
  if (bridge) {
    unsubscribe = bridge.subscribe("entity_changed", () => {
      void refreshGames();
    });
  }
});

onBeforeUnmount(() => {
  unsubscribe?.();
  unsubscribe = null;
});
</script>

<template>
  <div class="arrancador-shell">
    <AppTitlebar
      :sidebar-hidden="sidebarHidden"
      @toggle-sidebar="sidebarHidden = !sidebarHidden"
    >
      <template #right>
        <AppSpotlight v-model="search" />
      </template>
    </AppTitlebar>

    <div class="arrancador-shell__body">
      <AppSidebar
        :current="section"
        :hidden="sidebarHidden"
        @select="section = $event"
      />

      <div class="arrancador-content">
        <section class="arrancador-page">
          <h1 class="arrancador-page__title">
            {{
              section === "library"
                ? "Библиотека"
                : section === "catalogue"
                ? "Каталог"
                : section === "statistics"
                ? "Статистика"
                : "Настройки"
            }}
          </h1>

          <p v-if="section !== 'library'" class="arrancador-page__hint">
            Раздел пока доступен только в legacy Arrancador.exe. Phase 4
            охватывает миграцию библиотеки; остальные разделы — Phase 5+.
          </p>

          <template v-if="section === 'library'">
            <p class="arrancador-page__hint">
              Запуск игр и автоматический сканер пока работают через legacy
              Arrancador.exe (Phase 5). Здесь отображаются только game_obj,
              уже синхронизированные в ARK.
            </p>

            <div v-if="error" class="arrancador-error">{{ error }}</div>

            <div v-if="loading && games.length === 0" class="arrancador-empty">
              <p class="arrancador-empty__title">Загрузка…</p>
            </div>

            <div
              v-else-if="!loading && games.length === 0"
              class="arrancador-empty"
            >
              <p class="arrancador-empty__title">Библиотека пуста</p>
              <p class="arrancador-empty__hint">
                Добавьте игры через legacy Arrancador.exe — они появятся здесь
                автоматически после синхронизации в ARK.
              </p>
            </div>

            <div
              v-else-if="filteredGames.length === 0"
              class="arrancador-empty"
            >
              <p class="arrancador-empty__title">Ничего не найдено</p>
              <p class="arrancador-empty__hint">
                Сбросьте поисковый запрос, чтобы увидеть всю библиотеку.
              </p>
            </div>

            <div v-else class="arrancador-grid">
              <GameCard
                v-for="game in filteredGames"
                :key="game.id"
                :game="game"
              />
            </div>
          </template>
        </section>
      </div>
    </div>
  </div>
</template>
