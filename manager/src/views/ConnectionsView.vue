<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  Button,
  Dropdown,
  Modal,
  SettingsList,
  SettingsRow,
  SettingsToggleRow,
  TextInput,
} from "@kosmos/visuals";
import codewarsIcon from "../../../desktop/src/integrations/assets/codewars.svg";
import leetcodeIcon from "../../../desktop/src/integrations/assets/leetcode.svg";
import hevyIcon from "../assets/integrations/hevy.svg";
import togglTrackIcon from "../assets/integrations/toggl-track.svg";
import bigfrontendIcon from "../assets/integrations/bigfrontend.svg";
import greatfrontendIcon from "../assets/integrations/greatfrontend.svg";
import type { IntegrationProvider, IntegrationsSnapshot } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const snapshot = ref<IntegrationsSnapshot | null>(null);
type CredentialMap = Record<string, string>;
type SettingsPatch = { intervalMinutes?: number; syncOnStartup?: boolean };
const credential = ref<CredentialMap>({});
const busy = ref<string | null>(null);
const intervals = [0, 15, 60, 360, 1440] as const;
const selectedProvider = ref<string | null>(null);
const intervalLabels = {
  0: "Вручную",
  15: "15 минут",
  60: "Час",
  360: "6 часов",
  1440: "Раз в день",
} satisfies Record<number, string>;
const intervalOptions = intervals.map((value) => ({
  value,
  label: intervalLabels[value],
}));
const providerIcons = {
  hevy: hevyIcon,
  toggl: togglTrackIcon,
  leetcode: leetcodeIcon,
  codewars: codewarsIcon,
  greatfrontend: greatfrontendIcon,
  bigfrontend: bigfrontendIcon,
} satisfies Record<string, string>;
const selected = computed(() =>
  snapshot.value?.providers.find(
    (provider) => provider.id === selectedProvider.value,
  ),
);
async function load() {
  snapshot.value = await props.client.call(
    "getIntegrations",
    undefined,
    "integrations",
  );
}
async function act(
  provider: IntegrationProvider,
  action: "save" | "clear" | "sync",
) {
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
async function login(provider: "leetcode" | "greatfrontend") {
  busy.value = `${provider}:login`;
  await props.client.call(
    provider === "leetcode" ? "loginLeetCode" : "loginGreatFrontend",
    undefined,
    `integration:${provider}`,
  );
  busy.value = null;
  await load();
}
async function update(provider: IntegrationProvider, patch: SettingsPatch) {
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
        <span
          class="connection-card-status"
          :class="{
            'connection-card-status--connected': provider.hasCredential,
          }"
          aria-hidden="true"
        />
      </button>
    </div>

    <Modal
      :open="selected !== undefined"
      :title="selected ? `Настройка ${selected.label}` : 'Настройка интеграции'"
      width="min(720px, 94vw)"
      @close="closePanel"
    >
      <article v-if="selected" class="stack connection-panel">
        <SettingsList class-name="!bg-transparent">
          <SettingsRow title="Синхронизация">
            <template #control>
              <Dropdown
                :model-value="selected.settings.intervalMinutes"
                :options="intervalOptions"
                :searchable="false"
                @update:model-value="
                  (interval) =>
                    update(selected, { intervalMinutes: Number(interval) })
                "
              />
            </template>
          </SettingsRow>
          <SettingsToggleRow
            title="Синхронизировать при запуске"
            :model-value="selected.settings.syncOnStartup"
            @update:model-value="
              (syncOnStartup) => update(selected, { syncOnStartup })
            "
          />
          <SettingsRow
            v-if="selected.id !== 'leetcode' && selected.id !== 'greatfrontend'"
            :title="selected.credentialLabel"
          >
            <template #control>
              <TextInput
                v-model="credential[selected.id]"
                class="w-56"
                type="password"
                autocomplete="off"
                :placeholder="
                  selected.hasCredential
                    ? 'Оставьте пустым, чтобы не менять'
                    : 'Введите значение'
                "
              />
            </template>
          </SettingsRow>
        </SettingsList>
        <p v-if="selected.settings.lastError" class="error">
          {{ selected.settings.lastError }}
        </p>
        <div class="actions">
          <Button
            v-if="selected.id === 'leetcode' || selected.id === 'greatfrontend'"
            variant="surface"
            size="sm"
            :disabled="busy !== null"
            @click="login(selected.id)"
            >Войти в {{ selected.label }}</Button
          ><Button
            v-else-if="credential[selected.id]"
            variant="surface"
            size="sm"
            :disabled="busy !== null"
            @click="act(selected, 'save')"
            >Сохранить</Button
          ><Button
            v-if="selected.hasCredential"
            variant="surface"
            size="sm"
            :disabled="busy !== null"
            @click="act(selected, 'clear')"
            >Отключить</Button
          ><Button
            variant="surface"
            size="sm"
            :disabled="busy !== null || !selected.hasCredential"
            @click="act(selected, 'sync')"
            >Синхронизировать</Button
          >
        </div>
      </article>
    </Modal>
  </section>
</template>
