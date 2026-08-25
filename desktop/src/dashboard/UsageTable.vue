<script setup lang="ts">
import { computed } from "vue";
import { EmptyState } from "@kosmos/visuals";
import type { DashboardUsageRow } from "./types";

const props = defineProps<{
  rows: DashboardUsageRow[];
  loading: boolean;
}>();

const hasRows = computed(() => props.rows.length > 0);

function iconSrc(iconRef?: string | null): string | null {
  if (!iconRef) return null;
  if (/^(file|https?|data|kosmos-icon):/i.test(iconRef)) return iconRef;
  return `file:///${iconRef.replace(/\\/g, "/")}`;
}

const iconSrcMap = computed(() => {
  const map = new Map<string, string | null>();
  for (const row of props.rows) map.set(row.id, iconSrc(row.iconRef));
  return map;
});

function onIconError(event: Event): void {
// SAFETY: the surrounding domain validation preserves the asserted contract.
  (event.currentTarget as HTMLImageElement).hidden = true;
}

function fmtDuration(ms: number): string {
  const totalMinutes = Math.round(ms / 60000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours <= 0) return `${minutes} мин`;
  if (minutes === 0) return `${hours} ч`;
  return `${hours} ч ${minutes} мин`;
}

function fmtDate(iso?: string | null): string {
  if (!iso) return "—";
  try {
    const d = new Date(iso);
    return d.toLocaleString("ru", {
      year: "numeric",
      month: "short",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}
</script>

<template>
  <div class="wrap">
    <EmptyState v-if="loading" title="Загрузка…" compact />
    <EmptyState v-else-if="!hasRows" title="Затреканное время не найдено" compact />
    <div v-else class="usage-table">
      <div class="usage-table__header">
        <span class="usage-table__header-app">
          <span class="usage-table__header-icon" aria-hidden="true"></span>
          <span>Приложение</span>
        </span>
        <span>Всего</span>
        <span>Активно</span>
        <span>Запусков</span>
        <span>Последний запуск</span>
        <span>Путь</span>
      </div>
      <div class="usage-table__body kosmos-scroll">
        <div
          v-for="row in rows"
          :key="row.id"
          v-memo="[
            row.id,
            row.iconRef,
            row.displayName,
            row.processName,
            row.runtimeMs,
            row.foregroundMs,
            row.sessions,
            row.lastSeenAt,
            row.normalizedPath,
          ]"
          class="usage-row"
        >
          <div class="usage-row__app">
            <span class="app-icon" aria-hidden="true">
              <img
                v-if="iconSrcMap.get(row.id)"
                :src="iconSrcMap.get(row.id)!"
                alt=""
                draggable="false"
                @error="onIconError"
              />
            </span>
            <span class="usage-row__names">
              <span class="process-name">{{ row.displayName || row.processName }}</span>
            </span>
          </div>
          <div class="duration">{{ fmtDuration(row.runtimeMs) }}</div>
          <div>{{ fmtDuration(row.foregroundMs) }}</div>
          <div>{{ row.sessions }}</div>
          <time class="date" :datetime="row.lastSeenAt ?? undefined">
            {{ fmtDate(row.lastSeenAt) }}
          </time>
          <div class="path" :title="row.normalizedPath">{{ row.normalizedPath }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.wrap {
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.usage-table {
  display: flex;
  height: 100%;
  flex-direction: column;
  overflow: hidden;
  font-size: 13px;
}

.usage-table__header,
.usage-row {
  display: grid;
  grid-template-columns:
    minmax(220px, 1.6fr) minmax(92px, 0.48fr) minmax(92px, 0.48fr) minmax(76px, 0.34fr)
    minmax(150px, 0.72fr) minmax(220px, 1.2fr);
  gap: 16px;
  align-items: center;
}

.usage-table__header {
  padding: 8px 20px;
  border-bottom: 1px solid var(--border-color-strong);
  background: var(--main-background-color);
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.5px;
  text-transform: uppercase;
}

.usage-table__header-app {
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.usage-table__header-icon {
  width: 20px;
  justify-self: center;
}

.usage-table__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.usage-row {
  min-height: 36px;
  padding: 4px 20px;
  border-bottom: 1px solid color-mix(in srgb, var(--border-color-strong) 72%, transparent);
  color: var(--foreground);
}

.usage-row:hover {
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
}

.usage-row__app {
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.process-name {
  overflow: hidden;
  text-overflow: ellipsis;
}

.app-icon {
  display: inline-grid;
  place-items: center;
  width: 20px;
  height: 20px;
  justify-self: center;
  overflow: hidden;
  border-radius: 4px;
}

.usage-row__names {
  display: flex;
  min-width: 0;
  flex-direction: column;
}

.app-icon img {
  width: 20px;
  height: 20px;
  display: block;
  object-fit: cover;
}

.process-name,
.date,
.path {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.duration,
.process-name {
  font-weight: 500;
}

.date,
.path {
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  font-size: 12px;
}

.path {
  font-family: var(--font-mono, monospace);
}

@media (max-width: 1120px) {
  .usage-table__header,
  .usage-row {
    grid-template-columns:
      minmax(200px, 1.8fr) minmax(88px, 0.52fr) minmax(88px, 0.52fr)
      minmax(72px, 0.36fr) minmax(136px, 0.72fr);
  }

  .usage-table__header span:nth-child(6),
  .path {
    display: none;
  }
}

@media (max-width: 820px) {
  .usage-table__header,
  .usage-row {
    grid-template-columns: minmax(0, 1fr) 92px 72px;
  }

  .usage-table__header span:nth-child(3),
  .usage-table__header span:nth-child(5),
  .usage-row > div:nth-child(3),
  .date {
    display: none;
  }
}

@media (max-width: 620px) {
  .usage-table__header {
    display: none;
  }

  .usage-row {
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 10px;
    padding: 8px 12px;
  }

  .usage-row > div:nth-child(4) {
    display: none;
  }
}
</style>
