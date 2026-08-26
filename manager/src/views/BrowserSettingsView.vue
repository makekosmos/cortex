<script setup lang="ts">
import { onMounted, ref } from "vue";
import { SettingsList, SettingsToggleRow } from "@kosmos/visuals";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const persistData = ref(true);

async function load() {
  const settings = await props.client.call<{ persistData: boolean }>(
    "getBrowserSettings",
    undefined,
    "browser-settings",
  );
  if (settings) persistData.value = settings.persistData;
}

async function update(value: boolean) {
  const settings = await props.client.call<{ persistData: boolean }>(
    "setBrowserSettings",
    { persistData: value },
    "browser-settings",
  );
  if (settings) persistData.value = settings.persistData;
}

onMounted(() => void load());
</script>

<template>
  <section class="stack settings-page">
    <SettingsList>
      <SettingsToggleRow
        title="Хранить данные браузера"
        description="Сохраняет вход на сайтах, открытых через Kosmos, включая GitHub."
        :model-value="persistData"
        :label="persistData ? 'Хранить' : 'Не хранить'"
        @update:model-value="update"
      />
    </SettingsList>
  </section>
</template>
