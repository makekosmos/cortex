<script setup lang="ts">
// SecurityTab — DNS-резолвер для AI-провайдеров (диктация).

import { inject, onMounted } from "vue";
import {
  SettingsButtonRow,
  SettingsDropdownRow,
  SettingsList,
  SettingsTextInputRow,
} from "@kosmos/visuals";
import { DictationConfigKey, DNS_PROFILE_OPTIONS } from "../composables/useDictationConfig";

const ctx = inject(DictationConfigKey);
if (!ctx) throw new Error("SecurityTab requires DictationConfigKey provider in parent");

const {
  dictationConfig,
  dictationCustomDohUrl,
  dictationCustomDohError,
  dictationConnTestBusy,
  dictationConnTestResult,
  dictationConnReport,
  loadDictationConfig,
  onDictationDnsKindChange,
  onDictationCustomDohBlur,
  onDictationTestConnectivity,
} = ctx;

const STAGE_LABELS = {
  client_build: "Сборка HTTP клиента",
  dns_resolve: "DNS-резолв",
  tcp_connect: "TCP-коннект",
  http_head: "HTTPS-запрос",
} satisfies Record<string, string>;

const STAGE_HINTS = {
  client_build: "Невалидный DoH URL — проверьте поле выше.",
  dns_resolve: "DNS не работает. Попробуйте Cloudflare/Google DoH вместо «Системный».",
  tcp_connect: "TCP-коннект отвергнут. Возможен IP-блок.",
  http_head: "TLS handshake или HTTP не прошёл. Возможен SNI-блок.",
} satisfies Record<string, string>;

onMounted(() => {
  void loadDictationConfig();
});
</script>

<template>
  <div class="security-page kosmos-scroll">
    <SettingsList>
      <SettingsDropdownRow
        title="DNS-over-HTTPS"
        :model-value="dictationConfig.networkProfile.kind"
        :options="DNS_PROFILE_OPTIONS"
        @update:modelValue="onDictationDnsKindChange"
      />
      <SettingsTextInputRow
        v-if="dictationConfig.networkProfile.kind === 'custom_doh'"
        v-model="dictationCustomDohUrl"
        title="Свой DoH URL"
        :description="
          dictationCustomDohError
            ? `Ошибка: ${dictationCustomDohError}`
            : 'Пример: https://comss.dns.controld.com/dns-query'
        "
        placeholder="https://comss.dns.controld.com/dns-query"
        @blur="onDictationCustomDohBlur"
      />
      <SettingsButtonRow
        title="Проверить соединение"
        :description="
          dictationConnTestResult ||
          'Пошаговый probe к api.groq.com: client_build → dns_resolve → tcp_connect → http_head.'
        "
        variant="ghost"
        :loading="dictationConnTestBusy"
        :disabled="dictationConnTestBusy"
        :button-label="dictationConnTestBusy ? 'Проверяю…' : 'Проверить'"
        @click="onDictationTestConnectivity"
      />
    </SettingsList>

    <div v-if="dictationConnReport" class="conn-stages">
      <div
        v-for="stage in dictationConnReport.stages"
        :key="stage.name"
        class="conn-stage"
        :class="{
          'conn-stage--ok': stage.ok,
          'conn-stage--fail': !stage.ok,
          'conn-stage--first-failure': stage.name === dictationConnReport.firstFailure,
        }"
      >
        <div class="conn-stage__icon">{{ stage.ok ? "✓" : "✗" }}</div>
        <div class="conn-stage__body">
          <div class="conn-stage__label">
            {{ STAGE_LABELS[stage.name] ?? stage.name }}
            <span class="conn-stage__ms">{{ stage.ms }}ms</span>
            <span v-if="stage.ip" class="conn-stage__info">→ {{ stage.ip }}</span>
            <span v-if="stage.status" class="conn-stage__info">HTTP {{ stage.status }}</span>
          </div>
          <div v-if="!stage.ok && stage.error" class="conn-stage__error">
            {{ stage.error }}
          </div>
          <div
            v-if="stage.name === dictationConnReport.firstFailure && STAGE_HINTS[stage.name]"
            class="conn-stage__hint"
          >
            💡 {{ STAGE_HINTS[stage.name] }}
          </div>
        </div>
      </div>
      <div class="conn-stages__total">Итого: {{ dictationConnReport.totalMs }}ms</div>
    </div>
  </div>
</template>

<style scoped>
.conn-stages {
  margin-top: 16px;
  padding: 12px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.conn-stage {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 6px 0;
}

.conn-stage__icon {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  font-size: 0.6875rem;
  font-weight: 700;
}

.conn-stage--ok .conn-stage__icon {
  background: color-mix(in srgb, #4ade80 30%, transparent);
  color: #4ade80;
}

.conn-stage--fail .conn-stage__icon {
  background: color-mix(in srgb, #f5a524 30%, transparent);
  color: #f5a524;
}

.conn-stage__body {
  flex: 1;
  min-width: 0;
}

.conn-stage__label {
  font-size: 0.75rem;
  color: var(--foreground);
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: baseline;
}

.conn-stage__ms {
  font-size: 0.6875rem;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
}

.conn-stage__info {
  font-size: 0.6875rem;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
}

.conn-stage__error {
  font-size: 0.6875rem;
  color: #f5a524;
  margin-top: 2px;
  word-break: break-word;
}

.conn-stage__hint {
  font-size: 0.6875rem;
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  margin-top: 4px;
}

.conn-stages__total {
  font-size: 0.6875rem;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  margin-top: 4px;
  text-align: right;
}
</style>
