<script setup lang="ts">
// SecretsTab — API ключи для AI-провайдеров. Один header + plus + список
// провайдеров. Plus открывает AddApiKeyModal.
//
// Row layout: ProviderIcon · название провайдера · Toggle (on/off) ·
// ExternalLink (на консоль провайдера) · Trash (удалить).
// Toggle off → clear_api_key (ключ убирается из keyring).
// Toggle on без ключа → открыть AddApiKeyModal.

import { computed, inject, onMounted, ref } from "vue";
import { Toggle } from "@kosmos/visuals";
import { ExternalLink, Plus, Trash2 } from "@lucide/vue";
import { DictationConfigKey } from "../composables/useDictationConfig";
import AddApiKeyModal from "./AddApiKeyModal.vue";
import ProviderIcon from "./ProviderIcon.vue";

const ctx = inject(DictationConfigKey);
if (!ctx) throw new Error("SecretsTab requires DictationConfigKey provider in parent");

const { dictationHasApiKey, dictationApiKeyBusy, loadDictationConfig, onDictationClearApiKey } =
  ctx;

const modalOpen = ref(false);

const GROQ_CONSOLE_URL = "https://console.groq.com/keys";

/// Можно ли добавить новый ключ. Сейчас единственный провайдер Groq —
/// если у него уже есть ключ, плюс disabled. При расширении (Anthropic /
/// OpenAI и др.) проверять через список "providers без ключа".
const canAddMoreKeys = computed(() => !dictationHasApiKey.value);
const addDisabledTitle = computed(() =>
  canAddMoreKeys.value
    ? "Добавить ключ"
    : "Все провайдеры уже подключены. Чтобы заменить — удалите существующий.",
);

onMounted(() => {
  void loadDictationConfig();
});

function openAddModal() {
  if (!canAddMoreKeys.value) return;
  modalOpen.value = true;
}

function onSaved() {
  void loadDictationConfig();
}

function onToggleGroq(next: boolean) {
  if (next) {
    // Был off → on. Откроем модалку чтобы юзер ввёл ключ.
    if (!dictationHasApiKey.value) openAddModal();
  } else {
    // On → off. Удаляем ключ из keyring.
    void onDictationClearApiKey();
  }
}

function openConsole() {
  window.open(GROQ_CONSOLE_URL, "_blank", "noopener,noreferrer");
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

    <div v-if="!dictationHasApiKey" class="secrets-empty">Нет API-ключей</div>
    <div v-else class="secrets-list">
      <div class="secrets-item">
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
          :model-value="dictationHasApiKey"
          :disabled="dictationApiKeyBusy"
          aria-label="Включить Groq"
          @update:model-value="onToggleGroq"
        />
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
  font-size: 11px;
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
  font-size: 13px;
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
  font-size: 13px;
  font-weight: 500;
  color: var(--foreground);
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
