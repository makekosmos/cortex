<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Button, Modal, StatusDot } from "@kosmos/visuals";
import hevyIcon from "../assets/integrations/hevy.svg";
import togglTrackIcon from "../assets/integrations/toggl-track.svg";
import type { IntegrationProvider, IntegrationsSnapshot } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const snapshot = ref<IntegrationsSnapshot | null>(null);
const credential = ref<Record<string, string>>({});
const busy = ref<string | null>(null);
const intervals = [0, 15, 60, 360, 1440] as const;
const selectedProvider = ref<string | null>(null);
const intervalLabels: Record<number, string> = {
  0: "Вручную",
  15: "15 минут",
  60: "Час",
  360: "6 часов",
  1440: "Раз в день",
};
const providerIcons: Record<string, string> = {
  hevy: hevyIcon,
  toggl: togglTrackIcon,
};
const selected = computed(() =>
  snapshot.value?.providers.find((provider) => provider.id === selectedProvider.value),
);
async function load() {
  snapshot.value = await props.client.call("getIntegrations", undefined, "integrations");
}
async function act(provider: IntegrationProvider, action: "save" | "clear" | "sync") {
  busy.value = `${provider.id}:${action}`;
  if (action === "save")
    await props.client.call(
      "setIntegrationCredential",
      {
        provider: provider.id,
        credential: credential.value[provider.id] ?? "",
      },
      `integration:${provider.id}`,
    );
  if (action === "clear")
    await props.client.call(
      "clearIntegrationCredential",
      { provider: provider.id },
      `integration:${provider.id}`,
    );
  if (action === "sync")
    await props.client.call(
      "syncIntegrationNow",
      { provider: provider.id },
      `integration:${provider.id}`,
    );
  busy.value = null;
  await load();
}
async function loginLeetCode() {
  busy.value = "leetcode:login";
  await props.client.call("loginLeetCode", undefined, "integration:leetcode");
  busy.value = null;
  await load();
}
async function update(provider: IntegrationProvider, patch: Record<string, unknown>) {
  await props.client.call(
    "updateIntegrationSettings",
    { provider: provider.id, ...patch },
    `integration:${provider.id}`,
  );
  await load();
}
function closePanel() {
  selectedProvider.value = null;
  void load();
}
onMounted(load);
</script>

<template>
  <section class="stack connections-view">
    <p class="muted">
      Подключения работают через Engine и сохраняют данные только в его защищённом хранилище.
    </p>
    <div class="connections-grid">
      <button
        v-for="provider in snapshot?.providers ?? []"
        :key="provider.id"
        type="button"
        class="connection-card"
        :data-testid="`connection-card-${provider.id}`"
        :aria-label="`${provider.label}: ${provider.hasCredential ? 'Подключено' : 'Не подключено'}`"
        @click="selectedProvider = provider.id"
      >
        <img
          v-if="providerIcons[provider.id]"
          class="connection-card-logo"
          :src="providerIcons[provider.id]"
          alt=""
          aria-hidden="true"
        />
        <span v-else class="connection-card-mark" aria-hidden="true">
          {{ provider.label.slice(0, 2) }}
        </span>
        <strong class="connection-card-name">{{ provider.label }}</strong>
        <span class="connection-card-status">
          <span
            class="connection-card-status-dot"
            :class="{ 'connection-card-status-dot--connected': provider.hasCredential }"
          />
          {{ provider.hasCredential ? "Подключено" : "Не подключено" }}
        </span>
      </button>
    </div>

    <Modal
      :open="selected !== undefined"
      :title="selected ? `Настройка ${selected.label}` : 'Настройка подключения'"
      width="min(720px, 94vw)"
      @close="closePanel"
    >
      <article v-if="selected" class="stack connection-panel">
        <div class="card-heading">
          <div>
            <p class="eyebrow">Источник данных</p>
            <h2>{{ selected.label }}</h2>
          </div>
          <StatusDot
            :tone="selected.hasCredential ? 'success' : 'neutral'"
            :label="selected.hasCredential ? 'Подключено' : 'Не подключено'"
          />
        </div>
        <label class="field"
          ><span>Синхронизация</span
          ><select
            :value="selected.settings.intervalMinutes"
            @change="
              update(selected, {
                intervalMinutes: Number(($event.target as HTMLSelectElement).value),
              })
            "
          >
            <option v-for="interval in intervals" :key="interval" :value="interval">
              {{ intervalLabels[interval] }}
            </option>
          </select></label
        >
        <label class="field checkbox"
          ><input
            type="checkbox"
            :checked="selected.settings.syncOnStartup"
            @change="
              update(selected, { syncOnStartup: ($event.target as HTMLInputElement).checked })
            "
          /><span>Синхронизировать при запуске Engine</span></label
        >
        <label v-if="selected.id !== 'leetcode'" class="field"
          ><span>{{ selected.credentialLabel }}</span
          ><input
            v-model="credential[selected.id]"
            type="password"
            autocomplete="off"
            :placeholder="
              selected.hasCredential ? 'Оставьте пустым, чтобы не менять' : 'Введите значение'
            "
        /></label>
        <p v-if="selected.settings.lastError" class="error">{{ selected.settings.lastError }}</p>
        <div class="actions">
          <Button
            v-if="selected.id === 'leetcode'"
            size="sm"
            :disabled="busy !== null"
            @click="loginLeetCode"
            >Войти в LeetCode</Button
          ><Button
            v-else-if="credential[selected.id]"
            size="sm"
            :disabled="busy !== null"
            @click="act(selected, 'save')"
            >Сохранить</Button
          ><Button
            v-if="selected.hasCredential"
            size="sm"
            variant="ghost"
            :disabled="busy !== null"
            @click="act(selected, 'clear')"
            >Отключить</Button
          ><Button
            size="sm"
            variant="ghost"
            :disabled="busy !== null || !selected.hasCredential"
            @click="act(selected, 'sync')"
            >Синхронизировать</Button
          >
        </div>
      </article>
    </Modal>
  </section>
</template>
