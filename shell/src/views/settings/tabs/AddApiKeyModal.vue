<script setup lang="ts">
// AddApiKeyModal — добавление/обновление API ключа для AI-провайдера.
// Reference дизайн — Raycast "API Key" модалка (см. скриншот в задаче).
//
// Контракт:
//   - `open` — управляет видимостью (Modal v-model'ит through prop).
//   - На сохранение шлёт через composable `saveApiKey(key)` + emit'ит 'saved'.
//   - Verify — лёгкий GET /v1/models с переданным ключом, не сохраняет.

import { computed, inject, ref, watch } from "vue";
import { Button, Modal, SettingsDropdownRow, SettingsList, TextInput } from "@kosmos/visuals";
import { Eye, EyeOff } from "@lucide/vue";
import { DictationConfigKey } from "../composables/useDictationConfig";
import ProviderIcon from "./ProviderIcon.vue";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: []; saved: [] }>();

const ctx = inject(DictationConfigKey);
if (!ctx) throw new Error("AddApiKeyModal requires DictationConfigKey provider");

const { verifyApiKey, saveApiKey } = ctx;

// Phase 1 — только Groq. Архитектурно ready для расширения.
const PROVIDER_OPTIONS = [{ value: "groq", label: "Groq" }] as const;
type Provider = (typeof PROVIDER_OPTIONS)[number]["value"];

const provider = ref<Provider>("groq");
const apiKey = ref("");
const reveal = ref(false);
const verifyState = ref<"idle" | "verifying" | "ok" | "fail">("idle");
const verifyMessage = ref<string>("");
const saveState = ref<"idle" | "saving">("idle");

const providerConsoleUrl = computed(() => {
  switch (provider.value) {
    case "groq":
      return "https://console.groq.com/keys";
    default:
      return "";
  }
});

const canVerify = computed(
  () => apiKey.value.trim().length > 0 && verifyState.value !== "verifying",
);
const canSave = computed(
  () =>
    apiKey.value.trim().length > 0 &&
    saveState.value !== "saving" &&
    verifyState.value !== "verifying",
);

watch(
  () => props.open,
  (next) => {
    if (next) {
      // Reset на каждое открытие чтобы юзер не видел чужой ввод.
      apiKey.value = "";
      reveal.value = false;
      verifyState.value = "idle";
      verifyMessage.value = "";
      saveState.value = "idle";
    }
  },
);

async function onVerify() {
  const key = apiKey.value.trim();
  if (!key) return;
  verifyState.value = "verifying";
  verifyMessage.value = "";
  try {
    const r = await verifyApiKey(key);
    if (r.ok) {
      verifyState.value = "ok";
      verifyMessage.value = `Ключ принят · ${r.latencyMs ?? "?"}ms`;
    } else {
      verifyState.value = "fail";
      verifyMessage.value = r.error ?? "Не удалось проверить ключ";
    }
  } catch (e) {
    verifyState.value = "fail";
    verifyMessage.value = (e as Error)?.message ?? "Ошибка проверки";
  }
}

async function onSave() {
  const key = apiKey.value.trim();
  if (!key) return;
  saveState.value = "saving";
  try {
    await saveApiKey(key);
    emit("saved");
    emit("close");
  } catch (e) {
    verifyState.value = "fail";
    verifyMessage.value = (e as Error)?.message ?? "Ошибка сохранения";
  } finally {
    saveState.value = "idle";
  }
}

function onCancel() {
  emit("close");
}

function openProviderConsole() {
  if (providerConsoleUrl.value) {
    window.open(providerConsoleUrl.value, "_blank", "noopener,noreferrer");
  }
}

function onKeyEnter(e: KeyboardEvent) {
  // Ctrl+Enter — Save (как в Raycast). Просто Enter — Verify (если можно).
  if (e.key === "Enter") {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      if (canSave.value) void onSave();
    } else if (canVerify.value) {
      e.preventDefault();
      void onVerify();
    }
  }
}
</script>

<template>
  <Modal
    :open="open"
    title="API ключ"
    :width="'min(520px, 92vw)'"
    :hide-close="true"
    @close="onCancel"
  >
    <div class="add-api-key">
      <SettingsList>
        <SettingsDropdownRow
          title="Провайдер"
          :model-value="provider"
          :options="PROVIDER_OPTIONS.slice()"
          @update:model-value="(v: Provider) => (provider = v)"
        >
          <template #trigger-leading="{ option }">
            <ProviderIcon v-if="option" :provider="String(option.value)" :size="14" />
          </template>
          <template #option-leading="{ option }">
            <ProviderIcon :provider="String(option.value)" :size="14" />
          </template>
        </SettingsDropdownRow>
      </SettingsList>

      <p class="add-api-key__note">
        Запросы идут напрямую к провайдеру. Оплата — по тарифам провайдера. Ключ хранится в Windows
        Credential Manager.
      </p>

      <div class="add-api-key__key-block">
        <div class="add-api-key__key-label">API ключ</div>
        <div class="add-api-key__key-input">
          <TextInput
            v-model="apiKey"
            :type="reveal ? 'text' : 'password'"
            autocomplete="new-password"
            placeholder="Введите API ключ"
            :block="true"
            @keydown="onKeyEnter"
          />
          <button
            type="button"
            class="add-api-key__reveal"
            :aria-label="reveal ? 'Скрыть ключ' : 'Показать ключ'"
            @click="reveal = !reveal"
          >
            <component :is="reveal ? EyeOff : Eye" :size="16" />
          </button>
        </div>
        <div
          v-if="verifyMessage"
          class="add-api-key__verify-msg"
          :class="{
            'add-api-key__verify-msg--ok': verifyState === 'ok',
            'add-api-key__verify-msg--fail': verifyState === 'fail',
          }"
        >
          {{ verifyMessage }}
        </div>
      </div>

      <div class="add-api-key__actions">
        <Button
          class="add-api-key__console-btn"
          variant="ghost"
          size="sm"
          :disabled="!providerConsoleUrl"
          @click="openProviderConsole"
        >
          Открыть консоль ↗
        </Button>
        <Button
          variant="ghost"
          size="sm"
          :disabled="!canVerify"
          :loading="verifyState === 'verifying'"
          @click="onVerify"
        >
          {{ verifyState === "ok" ? "✓ Проверено" : "Проверить" }}
        </Button>
      </div>
    </div>

    <template #footer>
      <Button variant="ghost" size="sm" @click="onCancel">Отмена</Button>
      <Button
        variant="primary"
        size="sm"
        :disabled="!canSave"
        :loading="saveState === 'saving'"
        @click="onSave"
      >
        Сохранить
      </Button>
    </template>
  </Modal>
</template>

<style scoped>
.add-api-key {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.add-api-key__note {
  margin: 0;
  font-size: 0.6875rem;
  line-height: 1.4;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
}

.add-api-key__key-block {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.add-api-key__key-label {
  font-size: 0.8125rem;
  font-weight: 500;
  color: var(--foreground);
}

.add-api-key__key-input {
  position: relative;
  display: flex;
}

.add-api-key__key-input :deep(input) {
  padding-right: 36px;
}

.add-api-key__reveal {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  background: transparent;
  border: none;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 4px;
  border-radius: 4px;
}

.add-api-key__reveal:hover {
  color: var(--foreground);
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.add-api-key__verify-msg {
  font-size: 0.6875rem;
  line-height: 1.4;
}

.add-api-key__verify-msg--ok {
  color: #4ade80;
}

.add-api-key__verify-msg--fail {
  color: #f5a524;
}

.add-api-key__actions {
  display: flex;
  gap: 8px;
  align-items: stretch;
}

.add-api-key__actions :deep(.kosmos-btn) {
  padding-top: 16px;
  padding-bottom: 16px;
}

.add-api-key__console-btn {
  flex: 1;
}
</style>
