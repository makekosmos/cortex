<script setup lang="ts">
// ExportTab — каталог конвертеров + история экспортов. State и handlers
// инкапсулированы в composable `useExportTab`. При маунте тянем конвертеры.

import { onMounted } from "vue";
import LegacyRow from "../components/LegacyRow.vue";
import { useExportTab } from "../composables/useExportTab";
import type { ExportConverterInfo } from "@shared/ipc-types";

const {
  exportConverters,
  exportLoading,
  exportError,
  exportSelectedFormat,
  exportBusyId,
  exportStatusByConverter,
  exportHistory,
  loadExportConverters,
  onRunExport,
  formatHistoryTime,
} = useExportTab();

onMounted(() => {
  void loadExportConverters();
});

function runExport(c: ExportConverterInfo) {
  void onRunExport(c);
}
</script>

<template>
  <div v-if="exportError" class="error-banner">{{ exportError }}</div>

  <div v-if="exportLoading" class="empty">Загрузка…</div>

  <div v-else class="rows kosmos-scroll">
    <div v-if="exportConverters.length === 0" class="empty">
      Нет доступных конвертеров. Backend ещё не зарегистрировал ни одного.
    </div>

    <LegacyRow
      v-for="c in exportConverters"
      :key="c.converter_id"
      class="export-row"
      :title="c.display_name"
    >
      <template #hint>
        {{ c.object_type }} → {{ exportSelectedFormat[c.converter_id] ?? c.default_format }}
      </template>
      <template #extra>
        <div v-if="exportStatusByConverter[c.converter_id]" class="hint export-status">
          {{ exportStatusByConverter[c.converter_id] }}
        </div>
      </template>
      <div class="row-actions">
        <select
          v-if="c.supported_formats.length > 1"
          v-model="exportSelectedFormat[c.converter_id]"
          class="export-format-select"
          :disabled="exportBusyId === c.converter_id"
        >
          <option v-for="f in c.supported_formats" :key="f" :value="f">
            {{ f }}
          </option>
        </select>
        <button
          type="button"
          class="btn"
          :disabled="exportBusyId === c.converter_id"
          @click="runExport(c)"
        >
          {{ exportBusyId === c.converter_id ? "Экспорт…" : "Экспортировать" }}
        </button>
      </div>
    </LegacyRow>

    <div v-if="exportHistory.length > 0" class="ext-section-header">Последние экспорты</div>
    <LegacyRow v-for="(h, i) in exportHistory" :key="i" class="export-history-row">
      <template #title>
        {{ h.display_name }}
        <span class="hint">({{ h.format }})</span>
      </template>
      <template #hint>
        {{ formatHistoryTime(h.timestamp) }} · {{ h.file_count }} файлов ·
        {{ (h.bytes / 1024).toFixed(1) }} KB
      </template>
      <template #extra>
        <code class="value lock">{{ h.dest_dir }}</code>
      </template>
      <div class="row-actions">
        <span class="value" :class="{ muted: !h.ok }">
          {{ h.ok ? "ok" : "с ошибками" }}
        </span>
      </div>
    </LegacyRow>
  </div>
</template>

<style scoped>
/* Tab-specific CSS, мигрировано из родительского scoped-style SettingsView. */
.export-row .export-status {
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
  margin-top: 4px;
}

.export-format-select {
  font: inherit;
  font-size: 11px;
  padding: 4px 8px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  color: var(--foreground);
}

.export-history-row {
  opacity: 0.85;
}

.export-history-row .row-label code.value.lock {
  margin-top: 4px;
  max-width: 100%;
}
</style>
