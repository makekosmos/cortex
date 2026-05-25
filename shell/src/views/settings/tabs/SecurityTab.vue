<script setup lang="ts">
// SecurityTab — DNS-резолвер и proxy для AI-провайдеров (диктация).

import { inject, onMounted } from "vue";
import {
  RadioGroup,
  SettingsButtonRow,
  SettingsList,
  SettingsRow,
  SettingsTextInputRow,
} from "@kosmos/visuals";
import { DictationConfigKey, DNS_PROFILE_OPTIONS } from "../composables/useDictationConfig";

const ctx = inject(DictationConfigKey);
if (!ctx) throw new Error("SecurityTab requires DictationConfigKey provider in parent");

const {
  dictationConfig,
  dictationCustomDohUrl,
  dictationProxyInput,
  dictationConnTestBusy,
  dictationConnTestResult,
  loadDictationConfig,
  onDictationDnsKindChange,
  onDictationCustomDohBlur,
  onDictationProxyBlur,
  onDictationTestConnectivity,
} = ctx;

onMounted(() => {
  void loadDictationConfig();
});
</script>

<template>
  <div class="security-page kosmos-scroll">
    <p class="security-page__intro">
      Какой DNS-резолвер и прокси использовать для запросов к Groq и другим AI. В РФ Groq часто
      блокируется через DNS poisoning — DoH (DNS-over-HTTPS) это обходит без VPN. SNI / IP-блок DoH
      не лечит — для этого нужен прокси ниже.
    </p>
    <SettingsList>
      <SettingsRow
        title="DNS для AI-провайдеров"
        description="Резолвер только для dictation HTTP. Не влияет на остальной сетевой стек."
      >
        <template #control>
          <RadioGroup
            :model-value="dictationConfig.networkProfile.kind"
            :options="DNS_PROFILE_OPTIONS"
            name="dictation-dns"
            @update:modelValue="onDictationDnsKindChange"
          />
        </template>
      </SettingsRow>
      <SettingsTextInputRow
        v-if="dictationConfig.networkProfile.kind === 'custom_doh'"
        v-model="dictationCustomDohUrl"
        title="Свой DoH URL"
        description="Endpoint в формате https://example/dns-query."
        placeholder="https://comss.dns.controld.com/dns-query"
        @blur="onDictationCustomDohBlur"
      />
      <SettingsTextInputRow
        v-model="dictationProxyInput"
        title="HTTP / SOCKS proxy"
        description="Опционально. Если DoH недостаточно (SNI / IP блок). http://, https://, socks5://host:port. Пусто — без proxy."
        placeholder="socks5://127.0.0.1:1080"
        @blur="onDictationProxyBlur"
      />
      <SettingsButtonRow
        title="Проверить соединение"
        :description="
          dictationConnTestResult || 'HEAD-запрос к api.groq.com через выбранный DNS + proxy.'
        "
        variant="ghost"
        :loading="dictationConnTestBusy"
        :disabled="dictationConnTestBusy"
        :button-label="dictationConnTestBusy ? 'Проверяю…' : 'Проверить'"
        @click="onDictationTestConnectivity"
      />
    </SettingsList>
  </div>
</template>
