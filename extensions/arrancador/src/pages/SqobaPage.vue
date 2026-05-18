<script setup lang="ts">
// SqobaPage — per-game список backup'ов + кнопка «Создать бекап сейчас»
// + восстановление с confirmation.

import { ref, watch } from "vue";
import { ChevronDown, ChevronRight, Archive, RotateCcw } from "lucide-vue-next";

import { EmptyState, Modal } from "@kosmos/visuals";

import { useGames } from "../composables/useGames";
import {
  requireArrancadorApi,
  type SqobaBackup,
} from "../lib/arrancadorApi";

const { games, loading } = useGames();

interface GameState {
  expanded: boolean;
  backups: SqobaBackup[];
  loadingList: boolean;
  busy: boolean;
  error: string | null;
}

const gameStates = ref<Record<string, GameState>>({});

function ensureState(gameId: string): GameState {
  if (!gameStates.value[gameId]) {
    gameStates.value[gameId] = {
      expanded: false,
      backups: [],
      loadingList: false,
      busy: false,
      error: null,
    };
  }
  return gameStates.value[gameId];
}

function patchState(gameId: string, patch: Partial<GameState>) {
  const cur = ensureState(gameId);
  gameStates.value = {
    ...gameStates.value,
    [gameId]: { ...cur, ...patch },
  };
}

async function loadBackups(gameId: string) {
  patchState(gameId, { loadingList: true, error: null });
  try {
    const api = requireArrancadorApi();
    const res = await api.sqoba.list(gameId);
    patchState(gameId, { backups: res.backups ?? [], loadingList: false });
  } catch (cause) {
    patchState(gameId, {
      loadingList: false,
      error:
        cause instanceof Error ? cause.message : "Не удалось загрузить бэкапы",
    });
  }
}

async function toggleExpand(gameId: string) {
  const s = ensureState(gameId);
  const next = !s.expanded;
  patchState(gameId, { expanded: next });
  if (next && s.backups.length === 0 && !s.loadingList) {
    await loadBackups(gameId);
  }
}

async function onBackup(gameId: string) {
  if (ensureState(gameId).busy) return;
  patchState(gameId, { busy: true, error: null });
  try {
    const api = requireArrancadorApi();
    await api.sqoba.backup(gameId);
    await loadBackups(gameId);
  } catch (cause) {
    patchState(gameId, {
      error: cause instanceof Error ? cause.message : "Не удалось создать бэкап",
    });
  } finally {
    patchState(gameId, { busy: false });
  }
}

// Restore confirmation
const restoreModalOpen = ref(false);
const restoreTarget = ref<{ gameId: string; backup: SqobaBackup } | null>(null);
const restoring = ref(false);
const restoreError = ref<string | null>(null);
const restoreSuccess = ref<string | null>(null);

function openRestoreModal(gameId: string, backup: SqobaBackup) {
  restoreTarget.value = { gameId, backup };
  restoreError.value = null;
  restoreSuccess.value = null;
  restoreModalOpen.value = true;
}

function closeRestoreModal() {
  if (restoring.value) return;
  restoreModalOpen.value = false;
}

async function onConfirmRestore() {
  if (!restoreTarget.value) return;
  restoring.value = true;
  restoreError.value = null;
  try {
    const api = requireArrancadorApi();
    const res = await api.sqoba.restore(restoreTarget.value.backup.id);
    if (res.ok) {
      restoreSuccess.value = `Восстановлено файлов: ${res.restored_files ?? 0}`;
      setTimeout(() => {
        restoreModalOpen.value = false;
      }, 1200);
    } else {
      restoreError.value =
        (res.errors && res.errors.join("; ")) || "Восстановление не удалось";
    }
  } catch (cause) {
    restoreError.value =
      cause instanceof Error ? cause.message : "Ошибка восстановления";
  } finally {
    restoring.value = false;
  }
}

function formatTime(iso: string): string {
  try {
    return new Date(iso).toLocaleString("ru-RU", {
      day: "2-digit",
      month: "2-digit",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} Б`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} КБ`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} МБ`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} ГБ`;
}

// Reset states при изменении списка игр (если игра удалена — выкинем её state).
watch(games, (list) => {
  const ids = new Set(list.map((g) => g.id));
  const next: Record<string, GameState> = {};
  for (const [id, st] of Object.entries(gameStates.value)) {
    if (ids.has(id)) next[id] = st;
  }
  gameStates.value = next;
});
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">SQOBA</h1>
    <p class="arrancador-page__hint">
      Резервные копии сохранений (save-файлов) для каждой игры. Backup'ы
      хранятся локально в директории Kepler. Восстановление перезапишет
      текущие сохранения.
    </p>

    <EmptyState
      v-if="loading && games.length === 0"
      title="Загрузка библиотеки…"
    />

    <EmptyState
      v-else-if="games.length === 0"
      title="Нет игр в библиотеке"
      description="Запустите сканирование на вкладке «Сканер», чтобы появились игры."
    />

    <div v-else class="arrancador-sqoba-list">
      <article
        v-for="game in games"
        :key="game.id"
        class="arrancador-sqoba-game"
      >
        <header class="arrancador-sqoba-game__head">
          <button
            type="button"
            class="arrancador-sqoba-game__toggle"
            @click="toggleExpand(game.id)"
          >
            <ChevronDown
              v-if="gameStates[game.id]?.expanded"
              :size="14"
            />
            <ChevronRight v-else :size="14" />
            <span>{{ game.name }}</span>
          </button>
          <button
            type="button"
            class="arrancador-sqoba-game__action"
            :disabled="gameStates[game.id]?.busy"
            @click="onBackup(game.id)"
          >
            <Archive :size="14" />
            <span>{{
              gameStates[game.id]?.busy ? "Создаю…" : "Создать бекап"
            }}</span>
          </button>
        </header>

        <div
          v-if="gameStates[game.id]?.expanded"
          class="arrancador-sqoba-game__body"
        >
          <div
            v-if="gameStates[game.id]?.error"
            class="arrancador-error"
          >
            {{ gameStates[game.id]?.error }}
          </div>

          <div
            v-if="gameStates[game.id]?.loadingList"
            class="arrancador-sqoba-loading"
          >
            Загрузка списка…
          </div>

          <div
            v-else-if="(gameStates[game.id]?.backups.length ?? 0) === 0"
            class="arrancador-sqoba-empty"
          >
            Бэкапов пока нет.
          </div>

          <ul v-else class="arrancador-sqoba-backups">
            <li
              v-for="backup in gameStates[game.id]?.backups ?? []"
              :key="backup.id"
              class="arrancador-sqoba-backup"
            >
              <div class="arrancador-sqoba-backup__main">
                <span class="arrancador-sqoba-backup__time">{{
                  formatTime(backup.timestamp)
                }}</span>
                <span class="arrancador-sqoba-backup__meta">
                  {{ backup.files_count }} файл(ов) ·
                  {{ formatBytes(backup.bytes) }}
                </span>
              </div>
              <button
                type="button"
                class="arrancador-sqoba-backup__restore"
                @click="openRestoreModal(game.id, backup)"
              >
                <RotateCcw :size="13" />
                <span>Восстановить</span>
              </button>
            </li>
          </ul>
        </div>
      </article>
    </div>

    <Modal
      :open="restoreModalOpen"
      title="Восстановить сохранения?"
      @close="closeRestoreModal"
    >
      <div v-if="restoreTarget" class="arrancador-sqoba-restore">
        <p>
          Текущие save-файлы будут <strong>перезаписаны</strong> содержимым
          бэкапа от {{ formatTime(restoreTarget.backup.timestamp) }}.
        </p>
        <p class="arrancador-sqoba-restore__hint">
          Файлов в бэкапе: {{ restoreTarget.backup.files_count }} ·
          {{ formatBytes(restoreTarget.backup.bytes) }}
        </p>
        <div v-if="restoreError" class="arrancador-error">
          {{ restoreError }}
        </div>
        <div v-if="restoreSuccess" class="arrancador-sqoba-restore__ok">
          {{ restoreSuccess }}
        </div>
      </div>
      <template #footer>
        <button
          type="button"
          class="arrancador-sqoba-game__action"
          :disabled="restoring"
          @click="closeRestoreModal"
        >
          Отмена
        </button>
        <button
          type="button"
          class="arrancador-sqoba-game__action arrancador-sqoba-game__action--danger"
          :disabled="restoring"
          @click="onConfirmRestore"
        >
          {{ restoring ? "Восстанавливаю…" : "Восстановить" }}
        </button>
      </template>
    </Modal>
  </section>
</template>

<style scoped>
.arrancador-sqoba-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.arrancador-sqoba-game {
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: var(--card);
  overflow: hidden;
}

.arrancador-sqoba-game__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  gap: 12px;
}

.arrancador-sqoba-game__toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--foreground);
  background: transparent;
}

.arrancador-sqoba-game__action {
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

.arrancador-sqoba-game__action:not(:disabled):hover {
  background: var(--muted);
}

.arrancador-sqoba-game__action:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.arrancador-sqoba-game__action--danger {
  background: color-mix(in srgb, var(--destructive) 80%, transparent);
  color: var(--destructive-foreground, var(--foreground));
  border-color: color-mix(in srgb, var(--destructive) 60%, transparent);
}

.arrancador-sqoba-game__body {
  padding: 10px 12px 14px;
  border-top: 1px solid var(--border);
}

.arrancador-sqoba-loading,
.arrancador-sqoba-empty {
  font-size: 12px;
  color: var(--muted-foreground);
  padding: 4px 0;
}

.arrancador-sqoba-backups {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.arrancador-sqoba-backup {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--background);
}

.arrancador-sqoba-backup__main {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.arrancador-sqoba-backup__time {
  font-size: 12px;
  color: var(--foreground);
}

.arrancador-sqoba-backup__meta {
  font-size: 11px;
  color: var(--muted-foreground);
}

.arrancador-sqoba-backup__restore {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--card);
  color: var(--foreground);
  font-size: 12px;
}

.arrancador-sqoba-backup__restore:hover {
  background: var(--muted);
}

.arrancador-sqoba-restore {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.arrancador-sqoba-restore p {
  margin: 0;
  font-size: 13px;
}

.arrancador-sqoba-restore__hint {
  font-size: 12px;
  color: var(--muted-foreground);
}

.arrancador-sqoba-restore__ok {
  font-size: 12px;
  color: var(--accent, var(--foreground));
}
</style>
