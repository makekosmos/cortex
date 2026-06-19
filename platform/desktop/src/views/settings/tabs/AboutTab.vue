<script setup lang="ts">
// AboutTab — версия Kepler + кнопка «Проверить обновления».

import { computed, onMounted, ref } from "vue";
import { Button, SettingsList, SettingsRow } from "@kosmos/visuals";
import type { StorageSummary } from "@shared/ipc-types";
import AdvancedPageLayout, { type IntroDescriptor } from "../components/AdvancedPageLayout.vue";

defineProps<{
  intro: IntroDescriptor | null;
  version: string;
  checkResultLabel: string;
  updateChecking: boolean;
  isDownloading: boolean;
}>();

defineEmits<{ checkUpdates: [] }>();

const storageSummary = ref<StorageSummary | null>(null);
const storageError = ref("");

const visibleStorageItems = computed(() => storageSummary.value?.items ?? []);

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 Б";
  const units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value >= 100 || unit === 0 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

async function loadStorageSummary() {
  storageError.value = "";
  try {
    storageSummary.value = await window.kepler.settings.storageSummary();
  } catch (e) {
    storageError.value = (e as Error).message;
  }
}

onMounted(() => {
  void loadStorageSummary();
});
</script>

<template>
  <AdvancedPageLayout :intro="intro">
    <SettingsList>
      <SettingsRow title="Версия Kosmos" :description="checkResultLabel">
        <template #control>
          <div class="about-actions">
            <code class="about-value">{{ version }}</code>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              :disabled="updateChecking || isDownloading"
              @click="$emit('checkUpdates')"
            >
              {{ updateChecking ? "Проверяем…" : "Проверить обновления" }}
            </Button>
          </div>
        </template>
      </SettingsRow>
    </SettingsList>

    <h2 class="advanced-section-title">Данные</h2>
    <SettingsList>
      <SettingsRow
        title="Всего"
        :description="storageSummary ? storageSummary.dataDir : storageError || 'Считаю размер…'"
      >
        <template #control>
          <span>{{ storageSummary ? formatBytes(storageSummary.totalBytes) : "…" }}</span>
        </template>
      </SettingsRow>
      <SettingsRow
        v-if="storageSummary && storageSummary.userDataDir !== storageSummary.dataDir"
        title="Папка приложения"
        :description="storageSummary.userDataDir"
      >
        <template #control>
          <span>Electron</span>
        </template>
      </SettingsRow>
      <SettingsRow
        v-for="item in visibleStorageItems"
        :key="item.id"
        :title="item.label"
        :description="item.description ? `${item.description} ${item.path}` : item.path"
      >
        <template #control>
          <span>{{ formatBytes(item.bytes) }}</span>
        </template>
      </SettingsRow>
      <SettingsRow v-if="storageError" title="Ошибка подсчёта" :description="storageError" />
    </SettingsList>
  </AdvancedPageLayout>
</template>

<style scoped>
.about-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.about-value {
  color: var(--text-primary);
  font: inherit;
  font-weight: 600;
}
</style>
