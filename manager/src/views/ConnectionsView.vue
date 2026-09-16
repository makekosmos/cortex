<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Button, Modal, SettingsList, SettingsRow, Skeleton, TextInput } from "@kosmos/visuals";
import type {
  IntegrationProvider,
  IntegrationsSnapshot,
  StoreCatalogSnapshot,
} from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";
import { appIcon } from "../app-icons";
import {
  authMode,
  canLogin,
  credentialType,
  integrationCards,
  windowsListings,
  type ConnectionCard,
} from "../connection-helpers";
import { installTarget } from "../store-helpers";
import { useConnectionDrafts } from "../composables/useConnectionDrafts";
import { usePackageDisclosure } from "../composables/usePackageDisclosure";
import PermissionDisclosure from "./PermissionDisclosure.vue";

const props = defineProps<{ client: ManagerClient }>();
const snapshot = ref<IntegrationsSnapshot | null>(null);
const catalog = ref<StoreCatalogSnapshot | null>(null);
const loading = ref(true);
const { credential, settingKey, settingValue, setSettingValue, schema, canSave } =
  useConnectionDrafts();
const disclosure = usePackageDisclosure(props.client);
const busy = ref<string | null>(null);
const selectedProvider = ref<string | null>(null);
const cards = computed(() =>
  integrationCards(windowsListings(catalog.value?.listings ?? []), snapshot.value?.providers ?? []),
);
const selectedCard = computed(() => cards.value.find((card) => card.id === selectedProvider.value));
const selected = computed(() => selectedCard.value?.provider);
function icon(card: ConnectionCard) {
  return appIcon(card.id, card.iconUrl, card.iconPath);
}
function statusLabel(card: ConnectionCard) {
  if (!card.provider) return "Не установлена";
  const connected = card.provider.hasCredential ? "Подключено" : "Не подключено";
  return card.provider.enabled ? connected : `${connected} (пакет отключён)`;
}
function canInstall(card: ConnectionCard) {
  return (
    !card.provider &&
    catalog.value?.state === "fresh" &&
    card.listing !== undefined &&
    Boolean(installTarget(card.listing))
  );
}
async function load() {
  const [integrations, nextCatalog] = await Promise.all([
    props.client.call<IntegrationsSnapshot>(
      "getIntegrations",
      undefined,
      "connections-integrations",
    ),
    props.client.call<StoreCatalogSnapshot>("getStoreCatalog", undefined, "connections-catalog"),
  ]);
  if (integrations) snapshot.value = integrations;
  if (nextCatalog) catalog.value = nextCatalog;
  loading.value = false;
}
function select(card: ConnectionCard) {
  if (card.provider?.hasCredential) {
    selectedProvider.value = card.id;
    return;
  }
  const target = card.provider
    ? { package_id: card.provider.id, version: card.provider.packageVersion }
    : card.listing
      ? installTarget(card.listing)
      : null;
  if (!card.provider && !target) {
    selectedProvider.value = card.id;
    return;
  }
  void disclosure.request(target, card.label, () => {
    selectedProvider.value = card.id;
  });
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
async function install(card: ConnectionCard) {
  const target = card.listing ? installTarget(card.listing) : null;
  if (!target) return;
  busy.value = `${card.id}:install`;
  await props.client.call("installPackage", target, `connection-install:${card.id}`);
  busy.value = null;
  await load();
}
function closePanel() {
  selectedProvider.value = null;
  void load();
}
onMounted(async () => {
  await load();
  void Promise.all([
    props.client.call("refreshPackageCatalog", undefined, "connections-initial-package-catalog"),
    props.client.call("refreshStoreCatalog", undefined, "connections-initial-catalog"),
  ]).then(() => load());
});
</script>

<template>
  <section class="stack connections-view" aria-label="Интеграции">
    <div v-if="loading && !cards.length" class="connections-grid" aria-label="Загрузка интеграций">
      <Skeleton v-for="index in 6" :key="index" class="connection-card-skeleton" />
    </div>
    <div v-else-if="cards.length" class="connections-grid">
      <button
        v-for="card in cards"
        :key="card.id"
        type="button"
        class="connection-card"
        :data-testid="`connection-card-${card.id}`"
        :aria-label="`${card.label}: ${statusLabel(card)}`"
        @click="select(card)"
      >
        <img
          v-if="icon(card)"
          class="connection-card-logo"
          :src="icon(card)"
          alt=""
          aria-hidden="true"
        />
        <span v-else class="connection-card-mark" aria-hidden="true">
          {{ card.label.slice(0, 2) }}
        </span>
        <strong class="connection-card-name">{{ card.label }}</strong>
        <span
          class="connection-card-status"
          :class="{
            'connection-card-status--connected': Boolean(
              card.provider?.hasCredential && card.provider.enabled,
            ),
          }"
          aria-hidden="true"
        />
      </button>
    </div>
    <p v-else class="muted">Интеграции не найдены.</p>

    <Modal
      :open="selectedProvider !== null"
      :title="selectedCard ? `Настройка ${selectedCard.label}` : 'Настройка интеграции'"
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
      <article v-else-if="selectedCard" class="stack connection-panel">
        <p v-if="selectedCard.description" class="muted">{{ selectedCard.description }}</p>
        <div class="actions">
          <Button
            v-if="canInstall(selectedCard)"
            variant="surface"
            size="sm"
            :disabled="busy !== null"
            @click="install(selectedCard)"
            >{{ busy === `${selectedCard.id}:install` ? "Установка…" : "Установить" }}</Button
          >
          <p v-else class="muted">Установка сейчас недоступна.</p>
        </div>
      </article>
    </Modal>

    <PermissionDisclosure
      :open="disclosure.state.open"
      :name="disclosure.state.name"
      :sections="disclosure.state.sections"
      :unavailable="disclosure.state.unavailable"
      :busy="disclosure.state.busy"
      @confirm="disclosure.confirm"
      @decline="disclosure.decline"
    />
  </section>
</template>
