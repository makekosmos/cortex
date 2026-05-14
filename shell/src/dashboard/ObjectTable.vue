<script setup lang="ts">
import { computed } from "vue";
import type { DashboardObjectRow } from "./types";

const props = defineProps<{
  rows: DashboardObjectRow[];
  loading: boolean;
}>();

function fmtCreatedAt(iso: string): string {
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

const hasRows = computed(() => props.rows.length > 0);
</script>

<template>
  <div class="wrap">
    <div v-if="loading" class="state">Загрузка…</div>
    <div v-else-if="!hasRows" class="state">Объекты не найдены</div>
    <table v-else class="table">
      <thead>
        <tr>
          <th>Значение</th>
          <th>Тип</th>
          <th>Добавлено</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows" :key="row.id" @click="console.log('[dashboard] row', row)">
          <td class="primary">{{ row.primary }}</td>
          <td><code class="type-id">{{ row.typeId }}</code></td>
          <td class="created">{{ fmtCreatedAt(row.createdAt) }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.wrap {
  width: 100%;
  height: 100%;
  overflow: auto;
}

.state {
  padding: 48px 24px;
  text-align: center;
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
  font-size: 13px;
}

.table {
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 13px;
}

thead th {
  position: sticky;
  top: 0;
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

tbody tr {
  cursor: pointer;
  transition: background 0.1s var(--easing-standard);
}

tbody tr:hover {
  background: var(--surface);
}

tbody td {
  padding: 10px 12px;
  border-bottom: 1px solid
    color-mix(in srgb, var(--border) 60%, transparent);
  color: var(--foreground);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 240px;
}

td.primary {
  font-weight: 500;
}

.type-id {
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  padding: 2px 6px;
  border-radius: 4px;
}

td.created {
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  font-size: 12px;
}
</style>
