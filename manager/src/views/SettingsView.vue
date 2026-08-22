<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { SettingsList, SettingsToggleRow } from "@kosmos/visuals";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const enabled = ref(false);
const available = ref(false);
const label = computed(() => (enabled.value ? "Включён" : "Отключён"));

async function load() {
  const result = await props.client.call<{ enabled: boolean; available: boolean }>(
    "getAutostart",
    undefined,
    "autostart",
  );
  if (!result) return;
  enabled.value = result.enabled;
  available.value = result.available;
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
    </SettingsList>
    <p v-if="!available" class="muted">
      Настройка доступна только в установленном приложении Kosmos.
    </p>
  </section>
</template>
