<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { SettingsList, SettingsToggleRow } from "@kosmos/visuals";
import type { EngineSettings } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const settings = ref<EngineSettings | null>(null);
const warm = computed(() => (settings.value?.desktop_host.warm_timeout_seconds ?? 300) === 300);
const usageTracker = computed(() => settings.value?.usage_tracker.enabled ?? true);

async function refresh() {
  const nextSettings = await props.client.call<EngineSettings>(
    "getEngineSettings",
    undefined,
    "settings",
  );
  if (nextSettings) settings.value = nextSettings;
}
async function setWarm(enabled: boolean) {
  const next = await props.client.call<EngineSettings>("setWarmTimeout", { enabled }, "settings");
  if (next) settings.value = next;
}
async function setUsageTracker(enabled: boolean) {
  const next = await props.client.call<EngineSettings>("setUsageTracker", { enabled }, "settings");
  if (next) settings.value = next;
}
onMounted(() => void refresh());
</script>

<template>
  <section class="stack settings-page">
    <SettingsList>
      <SettingsToggleRow
        title="Тёплый рабочий стол"
        description="Сохранять движок готовым к работе 5 минут."
        :model-value="warm"
        :label="warm ? '5 минут' : 'Не держать'"
        @update:model-value="setWarm"
      />
      <SettingsToggleRow
        title="Учёт активности"
        description="Записывать активное приложение в ARK."
        :model-value="usageTracker"
        :label="usageTracker ? 'Включён' : 'Отключён'"
        @update:model-value="setUsageTracker"
      />
    </SettingsList>
  </section>
</template>
