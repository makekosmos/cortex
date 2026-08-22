<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  Button,
  HotkeyCapture,
  SettingsButtonRow,
  SettingsDropdownRow,
  SettingsList,
  SettingsRow,
  SettingsToggleRow,
  TextInput,
} from "@kosmos/visuals";
import type {
  DictationConfigPatch,
  DictationConfigSnapshot,
  DictationConnectivity,
  DictationLocalModels,
  DictationPendingItem,
  DictationProgressEvent,
  DictationStats,
} from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const snapshot = ref<DictationConfigSnapshot | null>(null);
const stats = ref<DictationStats | null>(null);
const models = ref<DictationLocalModels>([]);
const pending = ref<DictationPendingItem[]>([]);
const microphones = ref<Array<{ value: string; label: string }>>([
  { value: "", label: "Системный микрофон" },
]);
const apiKey = ref("");
const credentialMessage = ref("");
const connectivity = ref<DictationConnectivity | null>(null);
const modelInput = ref("");
const customDohUrl = ref("");
const captureAccelerator = ref<string | null>(null);
const captureCancelTick = ref(0);
const downloads = ref<Record<string, { label: string; percent: number | null }>>({});

const config = computed(() => snapshot.value?.config);
const languageOptions = [
  { value: "ru", label: "Русский" },
  { value: "en", label: "English" },
  { value: "auto", label: "Авто" },
];
const triggerOptions = [
  { value: "toggle", label: "Переключатель" },
  { value: "push_to_talk", label: "Удерживать" },
];
const injectOptions = [
  { value: "auto_paste", label: "Автовставка + буфер обмена" },
  { value: "clipboard_only", label: "Только буфер обмена" },
];
const providerOptions = [
  { value: "groq", label: "Groq" },
  { value: "local", label: "Локальная" },
  { value: "mock", label: "Тестовая" },
];
const networkOptions = [
  { value: "system", label: "Системный" },
  { value: "cloudflare_doh", label: "Cloudflare DoH" },
  { value: "google_doh", label: "Google DoH" },
  { value: "custom_doh", label: "Свой DoH" },
];
const idleOptions = [
  { value: 0, label: "Не выгружать" },
  { value: 60_000, label: "Через минуту" },
  { value: 300_000, label: "Через 5 минут" },
  { value: 1_800_000, label: "Через 30 минут" },
];

async function refresh() {
  const [next, nextStats, nextModels, nextPending] = await Promise.all([
    props.client.call<DictationConfigSnapshot>("getDictationConfig", undefined, "dictation-config"),
    props.client.call<DictationStats>("getDictationStats", undefined, "dictation-stats"),
    props.client.call<DictationLocalModels>(
      "listDictationLocalModels",
      undefined,
      "dictation-models",
    ),
    props.client.call<DictationPendingItem[]>(
      "listDictationPending",
      undefined,
      "dictation-pending",
    ),
  ]);
  if (next) {
    snapshot.value = next;
    modelInput.value = next.config.model;
    customDohUrl.value =
      next.config.networkProfile.kind === "custom_doh" ? next.config.networkProfile.url : "";
  }
  if (nextStats) stats.value = nextStats;
  if (nextModels) models.value = nextModels;
  if (nextPending) pending.value = nextPending;
}
async function patch(value: DictationConfigPatch) {
  const updated = await props.client.call<DictationConfigSnapshot>(
    "updateDictationConfig",
    value,
    "dictation-config",
  );
  if (updated) await refresh();
}
async function loadMicrophones() {
  try {
    const devices = await navigator.mediaDevices?.enumerateDevices();
    microphones.value = [
      { value: "", label: "Системный микрофон" },
      ...(devices ?? [])
        .filter((device) => device.kind === "audioinput")
        .map((device, index) => ({
          value: device.deviceId,
          label: device.label || `Микрофон ${index + 1}`,
        })),
    ];
  } catch {
    /* Browser permission is optional for this configuration surface. */
  }
}
function onEvent(event: DictationProgressEvent) {
  if (event.kind === "capture") {
    captureAccelerator.value = event.accelerator;
    if (event.cancelled) captureCancelTick.value += 1;
    return;
  }
  if (event.kind === "download") {
    downloads.value = {
      ...downloads.value,
      [event.modelId]: {
        label: event.percent === null ? "Скачивание…" : `Скачивание ${Math.round(event.percent)}%`,
        percent: event.percent,
      },
    };
    return;
  }
  const next = { ...downloads.value };
  delete next[event.modelId];
  downloads.value = next;
  void refresh();
}
async function verifyKey() {
  const result = await props.client.call<{ valid: boolean; message: string }>(
    "verifyDictationApiKey",
    { key: apiKey.value },
    "credential",
  );
  if (result) credentialMessage.value = result.valid ? "Ключ принят." : result.message;
}
async function saveKey() {
  const result = await props.client.call<{ saved: boolean }>(
    "setDictationApiKey",
    { key: apiKey.value },
    "credential",
  );
  if (result?.saved) {
    apiKey.value = "";
    credentialMessage.value = "Ключ сохранён в Windows Credential Manager.";
    await refresh();
  }
}
async function clearKey() {
  const result = await props.client.call<{ cleared: boolean }>(
    "clearDictationApiKey",
    undefined,
    "credential",
  );
  if (result?.cleared) {
    apiKey.value = "";
    credentialMessage.value = "Ключ удалён.";
    await refresh();
  }
}
async function testConnectivity() {
  const result = await props.client.call<DictationConnectivity>(
    "testDictationConnectivity",
    undefined,
    "connectivity",
  );
  if (result) connectivity.value = result;
}
async function downloadModel(modelId: string) {
  const result = await props.client.call<{ started: boolean }>(
    "downloadDictationLocalModel",
    { modelId, select: false },
    `download-${modelId}`,
  );
  if (result?.started)
    downloads.value = {
      ...downloads.value,
      [modelId]: { label: "Скачивание…", percent: null },
    };
}
async function useModel(modelId: string) {
  if (await props.client.call("useDictationLocalModel", { modelId }, `model-${modelId}`))
    await refresh();
}
async function deleteModel(modelId: string) {
  if (await props.client.call("deleteDictationLocalModel", { modelId }, `model-${modelId}`))
    await refresh();
}
async function retry(uuid: string) {
  if (await props.client.call("retryDictation", { uuid }, `pending-${uuid}`)) await refresh();
}
async function discard(uuid: string) {
  if (await props.client.call("discardDictation", { uuid }, `pending-${uuid}`)) await refresh();
}
async function retryAll() {
  if (await props.client.call("retryAllDictation", undefined, "pending-all")) await refresh();
}
async function discardAll() {
  if (await props.client.call("discardAllDictation", undefined, "pending-all")) await refresh();
}
async function startCapture() {
  captureAccelerator.value = null;
  await props.client.call("beginDictationHotkeyCapture", undefined, "hotkey");
}
async function endCapture() {
  await props.client.call("endDictationHotkeyCapture", undefined, "hotkey");
}

let stopEvents: (() => void) | null = null;
onMounted(() => {
  void refresh();
  void loadMicrophones();
  stopEvents = window.kosmosManager.onDictationEvent(onEvent);
});
onBeforeUnmount(() => stopEvents?.());
</script>

<template>
  <section class="stack" aria-label="Диктовка и AI">
    <template v-if="config">
      <SettingsList>
        <SettingsDropdownRow
          title="Микрофон"
          :model-value="config.microphoneDeviceId ?? ''"
          :options="microphones"
          @update:model-value="(value: string) => patch({ microphoneDeviceId: value || null })"
        />
        <SettingsDropdownRow
          title="Язык"
          :model-value="config.language"
          :options="languageOptions"
          @update:model-value="(value: string) => patch({ language: value })"
        />
        <SettingsDropdownRow
          title="Режим триггера"
          :model-value="config.triggerMode"
          :options="triggerOptions"
          @update:model-value="(value: 'toggle' | 'push_to_talk') => patch({ triggerMode: value })"
        />
        <SettingsRow title="Горячая клавиша"
          ><template #control
            ><HotkeyCapture
              :model-value="config.hotkey"
              external-capture
              :pending-accelerator="captureAccelerator"
              :pending-cancel="captureCancelTick"
              @capture-start="startCapture"
              @capture-end="endCapture"
              @update:model-value="(value: string) => patch({ hotkey: value })" /></template
        ></SettingsRow>
        <SettingsDropdownRow
          title="Вставка"
          :model-value="config.injectMode"
          :options="injectOptions"
          @update:model-value="
            (value: 'auto_paste' | 'clipboard_only') => patch({ injectMode: value })
          "
        />
        <SettingsToggleRow
          title="Приглушать звук"
          :model-value="config.duckAudioDuringRecording"
          @update:model-value="(value: boolean) => patch({ duckAudioDuringRecording: value })"
        />
        <SettingsDropdownRow
          title="Провайдер"
          :model-value="config.provider"
          :options="providerOptions"
          @update:model-value="(value: 'groq' | 'local' | 'mock') => patch({ provider: value })"
        />
        <SettingsToggleRow
          title="Провайдер включён"
          :model-value="config.providerEnabled"
          @update:model-value="(value: boolean) => patch({ providerEnabled: value })"
        />
        <SettingsRow title="Модель"
          ><template #control
            ><TextInput
              v-model="modelInput"
              @blur="patch({ model: modelInput.trim() || config.model })" /></template
        ></SettingsRow>
        <SettingsDropdownRow
          title="Выгрузка локальной модели"
          :model-value="config.localIdleUnloadMs"
          :options="idleOptions"
          @update:model-value="(value: number) => patch({ localIdleUnloadMs: value })"
        />
        <SettingsDropdownRow
          title="Сеть"
          :model-value="config.networkProfile.kind"
          :options="networkOptions"
          @update:model-value="
            (kind: 'system' | 'cloudflare_doh' | 'google_doh' | 'custom_doh') =>
              patch({
                networkProfile:
                  kind === 'custom_doh'
                    ? {
                        kind,
                        url: customDohUrl.trim() || 'https://cloudflare-dns.com/dns-query',
                      }
                    : { kind },
              })
          "
        />
        <SettingsRow v-if="config.networkProfile.kind === 'custom_doh'" title="Свой DoH URL"
          ><template #control
            ><TextInput
              v-model="customDohUrl"
              placeholder="https://…/dns-query"
              @blur="
                patch({
                  networkProfile: {
                    kind: 'custom_doh',
                    url: customDohUrl.trim(),
                  },
                })
              " /></template
        ></SettingsRow>
        <SettingsButtonRow
          title="Проверить соединение"
          button-label="Проверить"
          variant="ghost"
          @click="testConnectivity"
        />
      </SettingsList>
      <SettingsRow
        v-for="stage in connectivity?.stages"
        :key="stage.name"
        :title="stage.name"
        :description="`${stage.ok ? 'OK' : stage.error || 'Ошибка'} · ${stage.ms} мс`"
      />
      <SettingsList>
        <SettingsRow title="Всего слов" :description="String(stats?.totalWords ?? 0)" />
        <SettingsRow
          title="Время диктовки"
          :description="`${Math.round(stats?.totalSeconds ?? 0)} сек`"
        />
        <SettingsRow
          title="Сэкономлено"
          :description="`${Math.round(stats?.savedSeconds ?? 0)} сек`"
        />
      </SettingsList>
      <SettingsList>
        <SettingsRow
          title="Groq API ключ"
          :description="snapshot.hasApiKey ? 'Ключ настроен' : 'Ключ не настроен'"
          ><template #control
            ><TextInput
              v-model="apiKey"
              type="password"
              autocomplete="new-password"
              placeholder="Новый ключ" /></template
        ></SettingsRow>
        <SettingsRow
          v-if="credentialMessage"
          title="Статус ключа"
          :description="credentialMessage"
        />
        <SettingsButtonRow
          title="Проверить ключ"
          button-label="Проверить"
          variant="ghost"
          :disabled="!apiKey"
          @click="verifyKey"
        />
        <SettingsButtonRow
          title="Сохранить ключ"
          button-label="Сохранить"
          variant="ghost"
          :disabled="!apiKey"
          @click="saveKey"
        />
        <SettingsButtonRow
          title="Удалить ключ"
          button-label="Удалить"
          variant="ghost"
          :disabled="!snapshot.hasApiKey"
          @click="clearKey"
        />
      </SettingsList>
      <SettingsList>
        <SettingsRow
          v-for="model in models"
          :key="model.id"
          :title="model.name"
          :description="
            downloads[model.id]?.label ||
            (model.selected ? 'Выбрана' : model.downloaded ? 'Скачана' : model.description)
          "
          ><template #control
            ><div class="row">
              <Button
                v-if="!model.downloaded"
                size="sm"
                variant="ghost"
                @click="downloadModel(model.id)"
                >Скачать</Button
              ><Button
                v-else-if="!model.selected"
                size="sm"
                variant="ghost"
                @click="useModel(model.id)"
                >Использовать</Button
              ><Button
                v-if="model.downloaded"
                size="sm"
                variant="ghost"
                @click="deleteModel(model.id)"
                >Удалить</Button
              >
            </div></template
          ></SettingsRow
        >
      </SettingsList>
      <SettingsList>
        <SettingsButtonRow
          title="Повторить все"
          button-label="Повторить"
          variant="ghost"
          :disabled="!pending.length"
          @click="retryAll"
        />
        <SettingsButtonRow
          title="Удалить все"
          button-label="Удалить"
          variant="ghost"
          :disabled="!pending.length"
          @click="discardAll"
        />
        <SettingsRow
          v-for="item in pending"
          :key="item.uuid"
          :title="`${item.createdAt} · попыток ${item.attempts}`"
          :description="item.lastError || 'Ожидает повторной отправки'"
          ><template #control
            ><div class="row">
              <Button size="sm" variant="ghost" @click="retry(item.uuid)">Повторить</Button
              ><Button size="sm" variant="ghost" @click="discard(item.uuid)">Удалить</Button>
            </div></template
          ></SettingsRow
        >
        <SettingsRow v-if="!pending.length" title="Очередь пуста" />
      </SettingsList>
    </template>
    <Button variant="ghost" @click="refresh">Обновить</Button>
  </section>
</template>
