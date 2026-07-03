<script setup lang="ts">
// LibraryPage — отображение game_obj из ARK + запуск через arrancador.launch.
//
// Карточка остаётся `GameCard` (link на /game/:id); launch вынесен в
// отдельный action под карточкой, чтобы не конфликтовать с router-link.

import { computed, ref } from "vue";
import { Plus, Play } from "@lucide/vue";

import { EmptyState } from "@kosmos/visuals";

import GameCard from "../components/GameCard.vue";
import { useGames } from "../composables/useGames";
import { useSearchQuery } from "../composables/useSearchQuery";
import { requireArrancadorApi, type LaunchResult } from "../lib/arrancadorApi";
import type { ArrancadorGame } from "../lib/arkGames";

const { games, loading, error, refresh } = useGames();
const search = useSearchQuery();

const filteredGames = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return games.value;
  return games.value.filter((g) => g.name.toLowerCase().includes(q));
});

const addName = ref("");
const addExePath = ref("");
const addSavePath = ref("");
const addBusy = ref(false);
const addMessage = ref<string | null>(null);
const addIsError = ref(false);
const dragActive = ref(false);

function inferNameFromPath(path: string): string {
  const fileName = path.split(/[\\/]/).pop() ?? path;
  return fileName.replace(/\.(exe|lnk)$/i, "").trim() || "Новая игра";
}

function droppedPath(event: DragEvent): string | null {
  const file = event.dataTransfer?.files?.[0] as (File & { path?: string }) | undefined;
  return file?.path ?? null;
}

function onDrop(event: DragEvent) {
  dragActive.value = false;
  const path = droppedPath(event);
  if (!path) {
    addMessage.value = "Перетащите .exe или ярлык игры";
    addIsError.value = true;
    return;
  }
  addExePath.value = path;
  if (!addName.value.trim()) addName.value = inferNameFromPath(path);
  addMessage.value = "Путь добавлен. Проверьте название и сохраните игру.";
  addIsError.value = false;
}

async function onAddManual() {
  if (addBusy.value) return;
  addBusy.value = true;
  addMessage.value = null;
  addIsError.value = false;
  try {
    const api = requireArrancadorApi();
    const result = await api.addManual({
      name: addName.value.trim(),
      exePath: addExePath.value.trim(),
      savePaths: addSavePath.value.trim() ? [addSavePath.value.trim()] : [],
    });
    if (!result.ok) throw new Error(result.error ?? "Не удалось добавить игру");
    addName.value = "";
    addExePath.value = "";
    addSavePath.value = "";
    addMessage.value = "Игра добавлена";
    await refresh();
  } catch (cause) {
    addMessage.value = cause instanceof Error ? cause.message : "Не удалось добавить игру";
    addIsError.value = true;
  } finally {
    addBusy.value = false;
  }
}

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
      ...launchStates.value[id],
      ...patch,
    },
  };
}

// Launch разрешён если есть exePath, либо это Steam-источник
// (для steam:// URL exe_path не нужен).
function canLaunch(game: ArrancadorGame): boolean {
  return game.exePath !== null || game.source === "steam";
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
    const msg = cause instanceof Error ? cause.message : "Не удалось запустить";
    setLaunchState(game.id, { busy: false, message: msg, isError: true });
  }
}
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">Библиотека</h1>

    <form
      class="arrancador-add-game"
      :class="{ 'arrancador-add-game--drag': dragActive }"
      @submit.prevent="onAddManual"
      @dragenter.prevent="dragActive = true"
      @dragover.prevent="dragActive = true"
      @dragleave.prevent="dragActive = false"
      @drop.prevent="onDrop"
    >
      <div class="arrancador-add-game__drop">
        <Plus :size="16" />
        <span>Перетащите .exe или ярлык игры сюда</span>
      </div>
      <div class="arrancador-add-game__fields">
        <input
          v-model="addName"
          class="arrancador-add-game__input"
          type="text"
          placeholder="Название"
          :disabled="addBusy"
        />
        <input
          v-model="addExePath"
          class="arrancador-add-game__input arrancador-add-game__input--path"
          type="text"
          placeholder="C:\Games\Game\Game.exe или ярлык .lnk"
          :disabled="addBusy"
        />
        <input
          v-model="addSavePath"
          class="arrancador-add-game__input arrancador-add-game__input--path"
          type="text"
          placeholder="Папка сейвов, если авто-поиск не найдёт"
          :disabled="addBusy"
        />
        <button
          type="submit"
          class="arrancador-add-game__submit"
          :disabled="addBusy || !addName.trim() || !addExePath.trim()"
        >
          {{ addBusy ? "Добавляю…" : "Добавить" }}
        </button>
      </div>
      <div
        v-if="addMessage"
        class="arrancador-add-game__message"
        :class="{ 'arrancador-add-game__message--error': addIsError }"
      >
        {{ addMessage }}
      </div>
    </form>

    <div v-if="error" class="arrancador-error">{{ error }}</div>

    <EmptyState v-if="loading && games.length === 0" title="Загрузка…" />

    <EmptyState
      v-else-if="!loading && games.length === 0"
      title="Библиотека пуста"
      description="Перетащите .exe или ярлык выше либо запустите сканирование на вкладке «Сканер»."
    />

    <EmptyState
      v-else-if="filteredGames.length === 0"
      title="Ничего не найдено"
      description="Сбросьте поисковый запрос, чтобы увидеть всю библиотеку."
    />

    <div v-else class="arrancador-grid">
      <div v-for="game in filteredGames" :key="game.id" class="arrancador-library-cell">
        <GameCard :game="game" />
        <div class="arrancador-library-cell__actions">
          <button
            type="button"
            class="arrancador-library-cell__launch"
            :disabled="!canLaunch(game) || launchStates[game.id]?.busy"
            :title="!canLaunch(game) ? 'Не задан путь к исполняемому файлу' : 'Запустить игру'"
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
            'arrancador-library-cell__msg--error': launchStates[game.id]?.isError,
          }"
        >
          {{ launchStates[game.id]?.message }}
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.arrancador-add-game {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 18px;
  padding: 12px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-card);
  background: var(--card);
}

.arrancador-add-game--drag {
  border-color: var(--accent);
  background: color-mix(in srgb, var(--accent) 10%, var(--card));
}

.arrancador-add-game__drop {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--muted-foreground);
}

.arrancador-add-game__fields {
  display: grid;
  grid-template-columns: minmax(140px, 0.8fr) minmax(220px, 1.4fr) minmax(220px, 1.2fr) auto;
  gap: 8px;
}

.arrancador-add-game__input {
  min-width: 0;
  height: 32px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--background);
  color: var(--foreground);
  outline: none;
}

.arrancador-add-game__input:focus {
  border-color: var(--accent);
}

.arrancador-add-game__input--path {
  font-family: ui-monospace, SFMono-Regular, monospace;
  font-size: 12px;
}

.arrancador-add-game__submit {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 32px;
  padding: 0 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--accent, var(--card));
  color: var(--accent-foreground, var(--foreground));
  font-size: 12px;
  font-weight: 600;
}

.arrancador-add-game__submit:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.arrancador-add-game__message {
  font-size: 12px;
  color: var(--muted-foreground);
}

.arrancador-add-game__message--error {
  color: var(--destructive-foreground, var(--destructive));
}

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

@media (max-width: 900px) {
  .arrancador-add-game__fields {
    grid-template-columns: 1fr;
  }
}
</style>
