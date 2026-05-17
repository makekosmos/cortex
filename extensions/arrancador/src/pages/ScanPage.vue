<script setup lang="ts">
// ScanPage — кнопка «Сканировать сейчас» + история scan'ов + список
// найденных game_obj.

import { onMounted, ref } from "vue";
import { RefreshCw } from "lucide-vue-next";

import { EmptyState } from "@kepler/visuals";

import { useGames } from "../composables/useGames";
import { requireArrancadorApi, type ScanResult } from "../lib/arrancadorApi";

const { games, loading, error, refresh } = useGames();

const HISTORY_KEY = "arrancador-scan-history-v1";
const HISTORY_LIMIT = 10;

interface ScanHistoryEntry {
  timestamp: string;
  added: number;
  updated: number;
  skipped: number;
  errors: number;
}

const history = ref<ScanHistoryEntry[]>([]);
const busy = ref(false);
const lastResult = ref<ScanResult | null>(null);
const scanError = ref<string | null>(null);

function loadHistory(): ScanHistoryEntry[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(HISTORY_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (e): e is ScanHistoryEntry =>
        typeof e === "object" &&
        e !== null &&
        typeof (e as ScanHistoryEntry).timestamp === "string",
    );
  } catch {
    return [];
  }
}

function persistHistory(entries: ScanHistoryEntry[]) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(HISTORY_KEY, JSON.stringify(entries));
  } catch {
    // ignore
  }
}

onMounted(() => {
  history.value = loadHistory();
});

async function onScan() {
  if (busy.value) return;
  busy.value = true;
  scanError.value = null;
  try {
    const api = requireArrancadorApi();
    const result = await api.scan();
    lastResult.value = result;
    const entry: ScanHistoryEntry = {
      timestamp: new Date().toISOString(),
      added: result.added,
      updated: result.updated,
      skipped: result.skipped,
      errors: result.errors?.length ?? 0,
    };
    history.value = [entry, ...history.value].slice(0, HISTORY_LIMIT);
    persistHistory(history.value);
    // Подсвеживаем список игр, чтобы добавленные сразу появились.
    await refresh();
  } catch (cause) {
    scanError.value =
      cause instanceof Error ? cause.message : "Не удалось выполнить сканирование";
  } finally {
    busy.value = false;
  }
}

function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString("ru-RU", {
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
</script>

<template>
  <section class="arrancador-page">
    <h1 class="arrancador-page__title">Сканер</h1>

    <div class="arrancador-scan-actions">
      <button
        type="button"
        class="arrancador-scan-btn"
        :disabled="busy"
        @click="onScan"
      >
        <RefreshCw :size="16" :class="{ 'arrancador-scan-btn__spin': busy }" />
        <span>{{ busy ? "Сканирование…" : "Сканировать сейчас" }}</span>
      </button>
      <div v-if="lastResult" class="arrancador-scan-summary">
        Добавлено: <strong>{{ lastResult.added }}</strong>, обновлено:
        <strong>{{ lastResult.updated }}</strong>, пропущено:
        <strong>{{ lastResult.skipped }}</strong
        ><span v-if="lastResult.errors.length">
          , ошибок: <strong>{{ lastResult.errors.length }}</strong></span
        >.
      </div>
    </div>

    <div v-if="scanError" class="arrancador-error">{{ scanError }}</div>

    <div
      v-if="lastResult && lastResult.errors.length > 0"
      class="arrancador-scan-errors"
    >
      <div class="arrancador-scan-errors__title">Ошибки сканирования:</div>
      <ul class="arrancador-scan-errors__list">
        <li v-for="(err, i) in lastResult.errors" :key="i">{{ err }}</li>
      </ul>
    </div>

    <section v-if="history.length > 0" class="arrancador-scan-history">
      <h2 class="arrancador-scan-history__title">История</h2>
      <ul class="arrancador-scan-history__list">
        <li
          v-for="entry in history"
          :key="entry.timestamp"
          class="arrancador-scan-history__row"
        >
          <span class="arrancador-scan-history__time">{{
            formatTime(entry.timestamp)
          }}</span>
          <span class="arrancador-scan-history__counts">
            +{{ entry.added }} / ↻{{ entry.updated }} / ={{ entry.skipped
            }}<span v-if="entry.errors > 0"> / !{{ entry.errors }}</span>
          </span>
        </li>
      </ul>
    </section>

    <h2 class="arrancador-scan-found">Найдено в библиотеке</h2>

    <div v-if="error" class="arrancador-error">{{ error }}</div>

    <EmptyState v-if="loading && games.length === 0" title="Загрузка…" />

    <EmptyState
      v-else-if="games.length === 0"
      title="Игр пока нет"
      description="Нажмите «Сканировать сейчас», чтобы найти установленные игры из Steam / Epic / GOG."
    />

    <div v-else class="arrancador-scan-list">
      <article
        v-for="game in games"
        :key="game.id"
        class="arrancador-scan-list__row"
      >
        <span class="arrancador-scan-list__name">{{ game.name }}</span>
        <span class="arrancador-scan-list__path">
          {{ game.exePath ?? "exe-путь не задан" }}
        </span>
      </article>
    </div>
  </section>
</template>

<style scoped>
.arrancador-scan-actions {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 16px;
  flex-wrap: wrap;
}

.arrancador-scan-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 38px;
  padding: 0 18px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--accent, var(--card));
  color: var(--accent-foreground, var(--foreground));
  font-size: 13px;
  font-weight: 600;
  transition: background 120ms var(--easing-standard);
}

.arrancador-scan-btn:not(:disabled):hover {
  background: var(--muted);
}

.arrancador-scan-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.arrancador-scan-btn__spin {
  animation: arrancador-spin 1s linear infinite;
}

@keyframes arrancador-spin {
  to {
    transform: rotate(360deg);
  }
}

.arrancador-scan-summary {
  font-size: 13px;
  color: var(--muted-foreground);
}

.arrancador-scan-errors {
  margin-bottom: 16px;
  padding: 10px 12px;
  border: 1px solid color-mix(in srgb, var(--destructive) 40%, transparent);
  border-radius: var(--radius-input);
  background: color-mix(in srgb, var(--destructive) 8%, transparent);
  font-size: 12px;
}

.arrancador-scan-errors__title {
  font-weight: 600;
  margin-bottom: 4px;
}

.arrancador-scan-errors__list {
  margin: 0;
  padding-left: 18px;
  color: var(--muted-foreground);
}

.arrancador-scan-history {
  margin: 20px 0;
}

.arrancador-scan-history__title {
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
}

.arrancador-scan-history__list {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.arrancador-scan-history__row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--card);
  font-size: 12px;
}

.arrancador-scan-history__time {
  color: var(--muted-foreground);
}

.arrancador-scan-history__counts {
  font-family: ui-monospace, SFMono-Regular, monospace;
  color: var(--foreground);
}

.arrancador-scan-found {
  margin: 20px 0 12px;
  font-size: 14px;
  font-weight: 600;
}
</style>
