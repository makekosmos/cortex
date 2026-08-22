<script setup lang="ts">
import { onMounted, ref } from "vue";
import { SettingsList, SettingsRow } from "@kosmos/visuals";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const version = ref<string | null>(null);

onMounted(async () => {
  version.value = await props.client.call<string>("getAppVersion", undefined, "app-version");
});
</script>

<template>
  <section class="settings-page stack" aria-labelledby="about-heading">
    <h2 id="about-heading" class="visually-hidden">О приложении</h2>
    <SettingsList>
      <SettingsRow title="Версия Kosmos" description="Текущая версия приложения">
        <template #control>
          <code>{{ version ?? "Загрузка…" }}</code>
        </template>
      </SettingsRow>
      <SettingsRow title="Менеджер" description="Центр управления приложениями, данными и службами">
        <template #control><span>Kosmos Manager</span></template>
      </SettingsRow>
      <SettingsRow title="Платформа" description="Системная среда приложения">
        <template #control><span>Windows desktop</span></template>
      </SettingsRow>
    </SettingsList>
  </section>
</template>
