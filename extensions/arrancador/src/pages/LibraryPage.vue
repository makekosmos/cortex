<script setup lang="ts">
// LibraryPage — отображение game_obj из ARK + запуск через arrancador.launch.
//
// Карточка остаётся `GameCard` (link на /game/:id); launch вынесен в
// отдельный action под карточкой, чтобы не конфликтовать с router-link.

import { computed, ref } from "vue";
import { Play } from "lucide-vue-next";

import { EmptyState } from "@kosmos/visuals";

import GameCard from "../components/GameCard.vue";
import { useGames } from "../composables/useGames";
import { useSearchQuery } from "../composables/useSearchQuery";
import { requireArrancadorApi, type LaunchResult } from "../lib/arrancadorApi";
import type { ArrancadorGame } from "../lib/arkGames";

const { games, loading, error } = useGames();
const search = useSearchQuery();

const filteredGames = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return games.value;
  return games.value.filter((g) => g.name.toLowerCase().includes(q));
});

// Per-game launch state: id -> { busy, message, isError }
interface LaunchState {
  busy: boolean;
  message: string | null;
  isError: boolean;
}
const launchStates = ref<Record<string, LaunchState>>({});

function setLaunchState(id: string, patch: Partial<LaunchState>) {
  launchStates.value = {
    ...launchStates.value,
    [id]: {
      busy: false,
      message: null,
      isError: false,
      ...(launchStates.value[id] ?? {}),
      ...patch,
    },
  };
}

// Launch разрешён если есть exePath, либо это Steam-источник
// (для steam:// URL exe_path не нужен).
function canLaunch(game: ArrancadorGame): boolean {
  return game.exePath !== null || isSteamGame(game);
}

function isSteamGame(game: ArrancadorGame): boolean {
  // Эвристика: source хранится в propsJson, но мы его не проектируем
  // в ArrancadorGame. Используем rawgId/exePath как косвенный сигнал —
  // если exe_path отсутствует, разрешаем попытку запуска (backend сам
  // решит через steam:// URL fallback). Backend вернёт ok:false при провале.
  return game.exePath === null;
}

async function onLaunch(game: ArrancadorGame) {
  if (launchStates.value[game.id]?.busy) return;
  setLaunchState(game.id, { busy: true, message: null, isError: false });
  try {
    const api = requireArrancadorApi();
    const result: LaunchResult = await api.launch(game.id);
    if (result.ok) {
      setLaunchState(game.id, {
        busy: false,
        message: `Запущено (PID ${result.pid})`,
        isError: false,
      });
      // Auto-clear через 4s.
      setTimeout(() => {
        const cur = launchStates.value[game.id];
        if (cur && !cur.busy && !cur.isError) {
          setLaunchState(game.id, { message: null });
        }
      }, 4000);
    } else {
      setLaunchState(game.id, {
        busy: false,
        message: result.error || "Не удалось запустить",
        isError: true,
      });
    }
  } catch (cause) {
    const msg =
      cause instanceof Error ? cause.message : "Не удалось запустить";
    setLaunchState(game.id, { busy: false, message: msg, isError: true });
  }
}
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">Библиотека</h1>

    <div v-if="error" class="arrancador-error">{{ error }}</div>

    <EmptyState v-if="loading && games.length === 0" title="Загрузка…" />

    <EmptyState
      v-else-if="!loading && games.length === 0"
      title="Библиотека пуста"
      description="Запустите сканирование на вкладке «Сканер», чтобы найти установленные игры."
    />

    <EmptyState
      v-else-if="filteredGames.length === 0"
      title="Ничего не найдено"
      description="Сбросьте поисковый запрос, чтобы увидеть всю библиотеку."
    />

    <div v-else class="arrancador-grid">
      <div
        v-for="game in filteredGames"
        :key="game.id"
        class="arrancador-library-cell"
      >
        <GameCard :game="game" />
        <div class="arrancador-library-cell__actions">
          <button
            type="button"
            class="arrancador-library-cell__launch"
            :disabled="!canLaunch(game) || launchStates[game.id]?.busy"
            :title="
              !canLaunch(game)
                ? 'Не задан путь к исполняемому файлу'
                : 'Запустить игру'
            "
            @click="onLaunch(game)"
          >
            <Play :size="14" />
            <span>{{ launchStates[game.id]?.busy ? "Запуск…" : "Запустить" }}</span>
          </button>
        </div>
        <div
          v-if="launchStates[game.id]?.message"
          class="arrancador-library-cell__msg"
          :class="{
            'arrancador-library-cell__msg--error':
              launchStates[game.id]?.isError,
          }"
        >
          {{ launchStates[game.id]?.message }}
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.arrancador-library-cell {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.arrancador-library-cell__actions {
  display: flex;
  gap: 6px;
}

.arrancador-library-cell__launch {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--card);
  color: var(--foreground);
  font-size: 12px;
  transition: background 120ms var(--easing-standard);
}

.arrancador-library-cell__launch:not(:disabled):hover {
  background: var(--muted);
}

.arrancador-library-cell__launch:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.arrancador-library-cell__msg {
  font-size: 11px;
  color: var(--muted-foreground);
}

.arrancador-library-cell__msg--error {
  color: var(--destructive-foreground, var(--destructive));
}
</style>
