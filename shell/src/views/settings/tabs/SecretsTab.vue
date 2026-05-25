<script setup lang="ts">
// SecretsTab — API ключи для AI-провайдеров (хранятся в Windows Credential Manager).

import { inject, onMounted } from "vue";
import { Button, SettingsList, SettingsRow, TextInput } from "@kosmos/visuals";
import { DictationConfigKey } from "../composables/useDictationConfig";

const ctx = inject(DictationConfigKey);
if (!ctx) throw new Error("SecretsTab requires DictationConfigKey provider in parent");

const {
  dictationApiKeyInput,
  dictationApiKeyBusy,
  dictationApiKeyMsg,
  dictationHasApiKey,
  loadDictationConfig,
  onDictationSaveApiKey,
  onDictationClearApiKey,
} = ctx;

onMounted(() => {
  void loadDictationConfig();
});
</script>

<template>
  <div class="security-page kosmos-scroll">
    <p class="security-page__intro">
      API-ключи хранятся в Windows Credential Manager — изолированно от файлов конфигурации, не
      попадают в backup'ы и логи. Используются функциями, которым нужен соответствующий провайдер
      (сейчас — только диктация Groq).
    </p>
    <SettingsList>
      <SettingsRow title="Groq">
        <template #control>
          <div class="control-stack">
            <TextInput
              v-model="dictationApiKeyInput"
              type="password"
              autocomplete="new-password"
              :placeholder="dictationHasApiKey ? '••••••••' : 'gsk_...'"
              :block="false"
            />
            <Button
              variant="primary"
              size="sm"
              :loading="dictationApiKeyBusy"
              :disabled="dictationApiKeyBusy || !dictationApiKeyInput.trim()"
              @click="onDictationSaveApiKey"
            >
              Сохранить
            </Button>
            <Button
              v-if="dictationHasApiKey"
              variant="danger"
              size="sm"
              :loading="dictationApiKeyBusy"
              :disabled="dictationApiKeyBusy"
              @click="onDictationClearApiKey"
            >
              Удалить
            </Button>
          </div>
          <p v-if="dictationApiKeyMsg" class="control-hint">{{ dictationApiKeyMsg }}</p>
        </template>
      </SettingsRow>
    </SettingsList>
  </div>
</template>
