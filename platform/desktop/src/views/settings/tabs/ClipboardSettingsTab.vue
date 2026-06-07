<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import type { ClipboardHistorySettings, ClipboardHistoryStats } from "@shared/ipc-types";
import AdvancedPageLayout, { type IntroDescriptor } from "../components/AdvancedPageLayout.vue";
import LegacyRow from "../components/LegacyRow.vue";

defineProps<{ intro: IntroDescriptor | null }>();

const settings = ref<ClipboardHistorySettings | null>(null);
const stats = ref<ClipboardHistoryStats | null>(null);
const loading = ref(true);
const error = ref("");

const retentionDays = computed(() => settings.value?.retentionDays ?? 30);
const maxMegabytes = computed(() =>
  settings.value ? Math.round(settings.value.maxBytes / 1024 / 1024) : 512,
);
const storageLabel = computed(() => formatBytes(stats.value?.storageBytes ?? 0));
const oldestLabel = computed(() => {
  const oldest = stats.value?.oldestItemAt;
  if (!oldest) return "Нет записей";
  return new Intl.DateTimeFormat("ru-RU", {
    day: "2-digit",
    month: "long",
    year: "numeric",
  }).format(new Date(oldest));
});

async function loadClipboardSettings(): Promise<void> {
  loading.value = true;
  error.value = "";
  try {
    const [nextSettings, nextStats] = await Promise.all([
      window.kepler.clipboardHistory.settings(),
      window.kepler.clipboardHistory.stats(),
    ]);
    settings.value = nextSettings;
    stats.value = nextStats;
  } catch (e) {
    error.value = `Не удалось загрузить настройки: ${String((e as Error)?.message ?? e)}`;
  } finally {
    loading.value = false;
  }
}

async function updateRetentionDays(event: Event): Promise<void> {
  const days = numberFromEvent(event);
  if (days === null) return;
  await updateSettings({ retentionDays: days });
}

async function updateMaxMegabytes(event: Event): Promise<void> {
  const megabytes = numberFromEvent(event);
  if (megabytes === null) return;
  await updateSettings({ maxBytes: megabytes * 1024 * 1024 });
}

async function updateSettings(patch: Partial<ClipboardHistorySettings>): Promise<void> {
  error.value = "";
  try {
    settings.value = await window.kepler.clipboardHistory.updateSettings(patch);
    stats.value = await window.kepler.clipboardHistory.stats();
  } catch (e) {
    error.value = `Не удалось сохранить: ${String((e as Error)?.message ?? e)}`;
  }
}

function numberFromEvent(event: Event): number | null {
  const value = Number((event.target as HTMLInputElement).value);
  if (!Number.isFinite(value) || value < 0) return null;
  return Math.floor(value);
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} Б`;
  const units = ["КБ", "МБ", "ГБ"];
  let value = bytes / 1024;
  let unit = units[0]!;
  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index]!;
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${unit}`;
}

onMounted(() => {
  void loadClipboardSettings();
});
</script>

<template>
  <AdvancedPageLayout :intro="intro" body-class="rows">
    <div v-if="loading" class="empty">Загрузка…</div>
    <div v-else class="clipboard-settings">
      <LegacyRow
        title="Срок хранения"
        hint="0 = не удалять по сроку. Закрепленные записи остаются всегда."
      >
        <label class="clipboard-settings__number">
          <input
            :value="retentionDays"
            min="0"
            step="1"
            type="number"
            @change="updateRetentionDays"
          />
          <span>дней</span>
        </label>
      </LegacyRow>

      <LegacyRow
        title="Лимит места"
        hint="0 = без лимита по размеру. При переполнении удаляются старые незакрепленные записи."
      >
        <label class="clipboard-settings__number">
          <input
            :value="maxMegabytes"
            min="0"
            step="16"
            type="number"
            @change="updateMaxMegabytes"
          />
          <span>МБ</span>
        </label>
      </LegacyRow>

      <LegacyRow
        title="Текущий объём"
        :hint="`${stats?.itemCount ?? 0} записей · ${stats?.pinnedCount ?? 0} закреплено`"
      >
        <span class="clipboard-settings__value">{{ storageLabel }}</span>
      </LegacyRow>

      <LegacyRow title="Самая старая запись">
        <span class="clipboard-settings__value">{{ oldestLabel }}</span>
      </LegacyRow>

      <div v-if="error" class="error">{{ error }}</div>
    </div>
  </AdvancedPageLayout>
</template>

<style scoped>
.clipboard-settings {
  display: grid;
  gap: 10px;
}

.clipboard-settings__number {
  display: flex;
  align-items: center;
  gap: 8px;
  color: color-mix(in srgb, var(--foreground) 62%, transparent);
  font-size: 12px;
  font-weight: 650;
}

.clipboard-settings__number input {
  width: 92px;
  height: 30px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  color: var(--foreground);
  padding: 0 9px;
  font: inherit;
  outline: none;
}

.clipboard-settings__number input:focus {
  border-color: color-mix(in srgb, var(--accent) 58%, transparent);
}

.clipboard-settings__value {
  color: var(--foreground);
  font-size: 12px;
  font-weight: 700;
}
</style>
