<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Button, Modal, SettingsList, SettingsRow, TextInput } from "@kosmos/visuals";
import type { IntegrationProvider, IntegrationsSnapshot } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";
import { appIcon } from "../app-icons";

const props = defineProps<{ client: ManagerClient }>();
const snapshot = ref<IntegrationsSnapshot | null>(null);
type CredentialMap = Record<string, string>;
type Setting = NonNullable<IntegrationProvider["settingSchema"]>[number];
const credential = ref<CredentialMap>({});
const settingDraft = ref<CredentialMap>({});
const busy = ref<string | null>(null);
const selectedProvider = ref<string | null>(null);
function authMode(provider: IntegrationProvider) {
  return provider.authMode === "browser_login" || provider.authMode === "none"
    ? provider.authMode
    : "credential";
}
function credentialType(provider: IntegrationProvider) {
  return provider.credentialInputType === "text" ? "text" : "password";
}
function icon(provider: IntegrationProvider) {
  return appIcon(provider.id, undefined, provider.iconPath);
}
function canLogin(provider: IntegrationProvider) {
  return authMode(provider) === "browser_login" && Boolean(provider.loginCapability);
}
const selected = computed(() => {
  const provider = snapshot.value?.providers.find((item) => item.id === selectedProvider.value);
  return provider;
});
async function load() {
  snapshot.value = await props.client.call("getIntegrations", undefined, "integrations");
}
function settingKey(provider: IntegrationProvider, setting: Setting) {
  return `${provider.id}:${setting.key}`;
}
function settingValue(provider: IntegrationProvider, setting: Setting) {
  return (
    settingDraft.value[settingKey(provider, setting)] ?? provider.settingValues?.[setting.key] ?? ""
  );
}
function setSettingValue(provider: IntegrationProvider, setting: Setting, value: string) {
  settingDraft.value[settingKey(provider, setting)] = value;
}
function schema(provider: IntegrationProvider) {
  return provider.settingSchema ?? [];
}
function canSave(provider: IntegrationProvider) {
  return schema(provider).length > 0
    ? schema(provider).some((setting) => settingValue(provider, setting).trim())
    : Boolean(credential.value[provider.id]?.trim());
}
async function save(provider: IntegrationProvider) {
  busy.value = `${provider.id}:save`;
  const values = schema(provider);
  if (values.length > 0) {
    for (const setting of values) {
      const value = settingValue(provider, setting).trim();
      if (value)
        await props.client.call(
          "setIntegrationCredential",
          { provider: provider.id, setting: setting.key, credential: value },
          `integration:${provider.id}`,
        );
    }
  } else {
    await props.client.call(
      "setIntegrationCredential",
      { provider: provider.id, credential: credential.value[provider.id] ?? "" },
      `integration:${provider.id}`,
    );
  }
  busy.value = null;
  await load();
}
async function act(provider: IntegrationProvider, action: "clear" | "sync") {
  busy.value = `${provider.id}:${action}`;
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
async function login(provider: IntegrationProvider) {
  if (!provider.loginCapability) return;
  busy.value = `${provider.id}:login`;
  await props.client.call(
    "loginIntegration",
    { provider: provider.id },
    `integration:${provider.id}`,
  );
  busy.value = null;
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
        :aria-label="`${provider.label}: ${provider.hasCredential ? 'Подключено' : 'Не подключено'}${provider.enabled ? '' : ' (пакет отключён)'}`"
        @click="selectedProvider = provider.id"
      >
        <img
          v-if="icon(provider)"
          class="connection-card-logo"
          :src="icon(provider)"
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
            'connection-card-status--connected': provider.hasCredential && provider.enabled,
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
          <SettingsRow
            v-for="setting in schema(selected)"
            :key="setting.key"
            :title="setting.label"
            :description="setting.description"
          >
            <template #control>
              <TextInput
                :model-value="settingValue(selected, setting)"
                class="w-56"
                :type="setting.kind === 'text' ? 'text' : 'password'"
                autocomplete="off"
                :placeholder="setting.required ? 'Обязательное значение' : 'Введите значение'"
                @update:model-value="setSettingValue(selected, setting, String($event))"
              />
            </template>
          </SettingsRow>
          <SettingsRow
            v-if="authMode(selected) === 'credential' && schema(selected).length === 0"
            :title="selected.credentialLabel"
          >
            <template #control>
              <TextInput
                v-model="credential[selected.id]"
                class="w-56"
                :type="credentialType(selected)"
                autocomplete="off"
                :placeholder="
                  selected.hasCredential ? 'Оставьте пустым, чтобы не менять' : 'Введите значение'
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
            v-if="canLogin(selected)"
            variant="surface"
            size="sm"
            :disabled="busy !== null"
            @click="login(selected)"
            >Войти в {{ selected.label }}</Button
          ><Button
            v-if="(authMode(selected) === 'credential' || canLogin(selected)) && canSave(selected)"
            variant="surface"
            size="sm"
            :disabled="busy !== null"
            @click="save(selected)"
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
            :disabled="busy !== null || !selected.hasCredential || !selected.enabled"
            @click="act(selected, 'sync')"
            >Синхронизировать</Button
          >
        </div>
      </article>
    </Modal>
  </section>
</template>
