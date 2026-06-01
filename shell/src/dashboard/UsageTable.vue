<script setup lang="ts">
import { computed } from "vue";
import type { DashboardUsageRow } from "./types";

const props = defineProps<{
  rows: DashboardUsageRow[];
  loading: boolean;
}>();

const hasRows = computed(() => props.rows.length > 0);

function iconSrc(iconRef?: string | null): string | null {
  if (!iconRef) return null;
  if (/^(file|https?|data):/i.test(iconRef)) return iconRef;
  return `file:///${iconRef.replace(/\\/g, "/")}`;
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
    <div v-if="loading" class="state">Загрузка…</div>
    <div v-else-if="!hasRows" class="state">Затреканное время не найдено</div>
    <table v-else class="usage-table">
      <thead>
        <tr>
          <th>Название процесса</th>
          <th>Суммарное время</th>
          <th>Название</th>
          <th>Активно</th>
          <th>Запусков</th>
          <th>Idle</th>
          <th>Последний запуск</th>
          <th>Путь</th>
        </tr>
      </thead>
      <tbody class="kosmos-scroll">
        <tr v-for="row in rows" :key="row.id">
          <td class="process">
            <span class="app-icon" aria-hidden="true">
              <img
                v-if="iconSrc(row.iconRef)"
                :src="iconSrc(row.iconRef)!"
                alt=""
                draggable="false"
              />
            </span>
            <span class="process-name">{{ row.processName }}</span>
          </td>
          <td class="duration">{{ fmtDuration(row.runtimeMs) }}</td>
          <td>{{ row.displayName }}</td>
          <td>{{ fmtDuration(row.foregroundMs) }}</td>
          <td>{{ row.sessions }}</td>
          <td>{{ fmtDuration(row.idleMs) }}</td>
          <td class="date">{{ fmtDate(row.lastSeenAt) }}</td>
          <td class="path" :title="row.normalizedPath">{{ row.normalizedPath }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.wrap {
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.state {
  padding: 48px 24px;
  text-align: center;
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
  font-size: 13px;
}

.usage-table {
  width: 100%;
  height: 100%;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 13px;
  table-layout: fixed;
}

.usage-table th {
  background: var(--background);
  border-bottom: 1px solid var(--border);
  padding: 10px 12px;
  text-align: left;
  font-weight: 500;
  font-size: 12px;
  letter-spacing: 0.3px;
  text-transform: uppercase;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  z-index: 1;
}

.usage-table thead,
.usage-table tbody tr {
  display: table;
  width: 100%;
  table-layout: fixed;
}

.usage-table tbody {
  display: block;
  height: calc(100% - 37px);
  overflow: auto;
}

.usage-table tr {
  transition: background 0.1s var(--easing-standard);
}

.usage-table tbody tr:hover {
  background: var(--surface);
}

.usage-table td {
  padding: 10px 12px;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  color: var(--foreground);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 260px;
}

.process,
.duration {
  font-weight: 500;
}

.process {
  display: flex;
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
  width: 22px;
  height: 22px;
  flex: 0 0 22px;
}

.app-icon img {
  width: 18px;
  height: 18px;
  object-fit: contain;
}

.date,
.path {
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  font-size: 12px;
}

.path {
  font-family: var(--font-mono, monospace);
  max-width: 360px;
}
</style>
