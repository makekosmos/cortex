<script setup lang="ts">
// SecretsTab — API ключи для AI-провайдеров. Один header + plus + список
// провайдеров. Plus открывает AddApiKeyModal.
//
// Row layout: ProviderIcon · название провайдера · Toggle (on/off) ·
// ExternalLink (на консоль провайдера) · Trash (удалить).
// Toggle off → временно выключить provider, ключ остаётся в keyring.
// Toggle on без ключа → открыть AddApiKeyModal.

import { computed, inject, onMounted, ref } from "vue";
import { Button, TextInput, Toggle } from "@kosmos/visuals";
import { ExternalLink, Plus, Trash2 } from "@lucide/vue";
import { DictationConfigKey } from "../composables/useDictationConfig";
import AddApiKeyModal from "./AddApiKeyModal.vue";
import ProviderIcon from "./ProviderIcon.vue";

const ctx = inject(DictationConfigKey);
if (!ctx) throw new Error("SecretsTab requires DictationConfigKey provider in parent");

const { dictationHasApiKey, dictationApiKeyBusy, loadDictationConfig, onDictationClearApiKey } =
  ctx;
const { dictationConfig, onDictationProviderEnabledChange } = ctx;

const modalOpen = ref(false);
const rawgHasApiKey = ref(false);
const rawgApiKeyInput = ref("");
const rawgApiKeyBusy = ref(false);
const rawgApiKeyMessage = ref("");

const GROQ_CONSOLE_URL = "https://console.groq.com/keys";

/// Можно ли добавить новый ключ. Сейчас единственный провайдер Groq —
/// если у него уже есть ключ, плюс disabled. При расширении (Anthropic /
/// OpenAI и др.) проверять через список "providers без ключа".
const canAddMoreKeys = computed(() => !dictationHasApiKey.value);
const hasAnyKey = computed(() => dictationHasApiKey.value || rawgHasApiKey.value);
const addDisabledTitle = computed(() =>
  canAddMoreKeys.value
    ? "Добавить ключ"
    : "Все провайдеры уже подключены. Чтобы заменить — удалите существующий.",
);

onMounted(() => {
  void loadDictationConfig();
  void loadRawgStatus();
});

async function loadRawgStatus() {
  const config = (await window.kepler.ark.request("arrancador.config.get", {})) as {
    rawg_api_key_set?: boolean;
  };
  rawgHasApiKey.value = config.rawg_api_key_set === true;
}

async function saveRawgApiKey() {
  const key = rawgApiKeyInput.value.trim();
  if (!key) return;
  rawgApiKeyBusy.value = true;
  rawgApiKeyMessage.value = "";
  try {
    await window.kepler.ark.request("arrancador.config.set_rawg_key", { key });
    rawgApiKeyInput.value = "";
    rawgApiKeyMessage.value = "Ключ RAWG сохранён";
    await loadRawgStatus();
  } catch (error) {
    rawgApiKeyMessage.value = `Ошибка: ${(error as Error).message}`;
  } finally {
    rawgApiKeyBusy.value = false;
  }
}

async function clearRawgApiKey() {
  rawgApiKeyBusy.value = true;
  rawgApiKeyMessage.value = "";
  try {
    await window.kepler.ark.request("arrancador.config.clear_rawg_key", {});
    rawgApiKeyMessage.value = "Ключ RAWG удалён";
    await loadRawgStatus();
  } catch (error) {
    rawgApiKeyMessage.value = `Ошибка: ${(error as Error).message}`;
  } finally {
    rawgApiKeyBusy.value = false;
  }
}

function openAddModal() {
  if (!canAddMoreKeys.value) return;
  modalOpen.value = true;
}

function onSaved() {
  void loadDictationConfig();
}

function onToggleGroq(next: boolean) {
  if (next) {
    if (!dictationHasApiKey.value) openAddModal();
    else void onDictationProviderEnabledChange(true);
  } else {
    void onDictationProviderEnabledChange(false);
  }
}

function openConsole() {
  window.open(GROQ_CONSOLE_URL, "_blank", "noopener,noreferrer");
}

function openRawgConsole() {
  window.open("https://rawg.io/apidocs", "_blank", "noopener,noreferrer");
}
</script>

<template>
  <div class="security-page kosmos-scroll">
    <div class="secrets-header">
      <p class="secrets-header__note">
        API-ключи хранятся в Windows Credential Manager. Оплата — по тарифам провайдера.
      </p>
      <button
        type="button"
        class="secrets-header__add"
        :aria-label="addDisabledTitle"
        :title="addDisabledTitle"
        :disabled="!canAddMoreKeys"
        @click="openAddModal"
      >
        <Plus :size="16" />
      </button>
    </div>

    <div v-if="!hasAnyKey" class="secrets-empty">Нет API-ключей</div>
    <div class="secrets-list">
      <div v-if="dictationHasApiKey" class="secrets-item">
        <ProviderIcon provider="groq" :size="18" />
        <div class="secrets-item__main">
          <div class="secrets-item__provider">Groq</div>
        </div>
        <button
          type="button"
          class="secrets-item__icon-btn"
          aria-label="Удалить ключ"
          title="Удалить ключ"
          :disabled="dictationApiKeyBusy"
          @click="onDictationClearApiKey"
        >
          <Trash2 :size="14" />
        </button>
        <button
          type="button"
          class="secrets-item__icon-btn"
          aria-label="Открыть консоль Groq"
          title="Открыть консоль Groq"
          @click="openConsole"
        >
          <ExternalLink :size="14" />
        </button>
        <Toggle
          :model-value="dictationHasApiKey && dictationConfig.providerEnabled"
          :disabled="dictationApiKeyBusy"
          aria-label="Включить Groq"
          @update:model-value="onToggleGroq"
        />
      </div>
      <div class="secrets-item secrets-item--rawg">
        <ProviderIcon provider="rawg" :size="18" />
        <div class="secrets-item__main">
          <div class="secrets-item__provider">RAWG</div>
          <div v-if="!rawgHasApiKey" class="secrets-item__hint">Для поиска игр в Arcadia</div>
          <div v-if="rawgApiKeyMessage" class="secrets-item__message">{{ rawgApiKeyMessage }}</div>
        </div>
        <template v-if="rawgHasApiKey">
          <button
            type="button"
            class="secrets-item__icon-btn"
            aria-label="Удалить ключ RAWG"
            title="Удалить ключ RAWG"
            :disabled="rawgApiKeyBusy"
            @click="clearRawgApiKey"
          >
            <Trash2 :size="14" />
          </button>
          <button
            type="button"
            class="secrets-item__icon-btn"
            aria-label="Открыть RAWG"
            title="Открыть RAWG"
            @click="openRawgConsole"
          >
            <ExternalLink :size="14" />
          </button>
        </template>
        <template v-else>
          <TextInput
            v-model="rawgApiKeyInput"
            class="secrets-item__key-input"
            type="password"
            autocomplete="new-password"
            placeholder="API-ключ RAWG"
            @keydown.enter="saveRawgApiKey"
          />
          <Button
            variant="primary"
            size="sm"
            :disabled="!rawgApiKeyInput.trim()"
            :loading="rawgApiKeyBusy"
            @click="saveRawgApiKey"
          >
            Сохранить
          </Button>
        </template>
      </div>
    </div>

    <AddApiKeyModal :open="modalOpen" @close="modalOpen = false" @saved="onSaved" />
  </div>
</template>

<style scoped>
.secrets-header {
  display: flex;
  align-items: center;
  gap: 12px;
}

.secrets-header__note {
  margin: 0;
  flex: 1;
  font-size: 0.6875rem;
  line-height: 1.4;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
}

.secrets-header__add {
  flex-shrink: 0;
  width: 24px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 6px;
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  padding: 0;
  transition:
    background 80ms ease,
    color 80ms ease,
    opacity 80ms ease;
}

.secrets-header__add:hover:not(:disabled) {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
  color: var(--foreground);
}

.secrets-header__add:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.secrets-empty {
  padding: 24px 16px;
  border-radius: 10px;
  border: 1px dashed color-mix(in srgb, var(--foreground) 15%, transparent);
  background: color-mix(in srgb, var(--foreground) 3%, transparent);
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  font-size: 0.8125rem;
  text-align: center;
}

.secrets-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.secrets-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 12px;
  /* Тот же фон что у SettingsList (`DNS-over-HTTPS` и др. кнопки настроек) —
   * иначе visual mismatch между табами. */
  background: var(--settings-list-background);
}

.secrets-item__main {
  flex: 1;
  min-width: 0;
}

.secrets-item__provider {
  font-size: 0.8125rem;
  font-weight: 500;
  color: var(--foreground);
}

.secrets-item__hint,
.secrets-item__message {
  margin-top: 2px;
  font-size: 0.6875rem;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
}

.secrets-item__key-input {
  width: 180px;
}

/* Иконочные кнопки (ExternalLink / Trash) — без фона и border'а по дефолту,
 * только на hover'е подсветка. */
.secrets-item__icon-btn {
  flex-shrink: 0;
  width: 26px;
  height: 26px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 6px;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
  padding: 0;
  transition:
    background 80ms ease,
    color 80ms ease;
}

.secrets-item__icon-btn:hover:not(:disabled) {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
  color: var(--foreground);
}

.secrets-item__icon-btn:disabled {
  opacity: 0.4;
  cursor: default;
}
</style>
