<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { SettingsList, SettingsToggleRow } from "@kosmos/visuals";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const enabled = ref(false);
const available = ref(false);
const trayIcon = ref(true);
const label = computed(() => (enabled.value ? "Включён" : "Отключён"));

type TraySettingsBridge = {
  trayIcon: { get(): Promise<boolean>; set(enabled: boolean): Promise<void> };
};

function trayBridge() {
  return (window as typeof window & { kepler?: { settings?: TraySettingsBridge } }).kepler?.settings;
}

async function load() {
  const result = await props.client.call<{ enabled: boolean; available: boolean }>(
    "getAutostart",
    undefined,
    "autostart",
  );
  if (!result) return;
  enabled.value = result.enabled;
  available.value = result.available;
  try {
    const bridge = trayBridge();
    trayIcon.value = bridge
      ? await bridge.trayIcon.get()
      : localStorage.getItem("kosmos.trayIcon") !== "false";
  } catch {
    trayIcon.value = true;
  }
}

async function setAutostart(value: boolean) {
  const result = await props.client.call<{ enabled: boolean; available: boolean }>(
    "setAutostart",
    { enabled: value },
    "autostart",
  );
  if (!result) return;
  enabled.value = result.enabled;
  available.value = result.available;
}

async function setTrayIcon(value: boolean) {
  trayIcon.value = value;
  const bridge = trayBridge();
  try {
    if (bridge) await bridge.trayIcon.set(value);
    else localStorage.setItem("kosmos.trayIcon", String(value));
  } catch {
    localStorage.setItem("kosmos.trayIcon", String(value));
  }
}

onMounted(() => void load());
</script>

<template>
  <section class="stack settings-page">
    <SettingsList>
      <SettingsToggleRow
        title="Запускать Kosmos при входе в систему"
        description="Автоматически запускать приложение Kosmos после входа в Windows."
        :model-value="enabled"
        :label="label"
        :disabled="!available"
        @update:model-value="setAutostart"
      />
      <SettingsToggleRow
        title="Показывать Kosmos в системном трее"
        description="Управляет значком Kosmos в области уведомлений Windows."
        :model-value="trayIcon"
        :label="trayIcon ? 'Показывать' : 'Скрывать'"
        @update:model-value="setTrayIcon"
      />
    </SettingsList>
  </section>
</template>
