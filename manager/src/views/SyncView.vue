<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  Button,
  Modal,
  SettingsButtonRow,
  SettingsList,
  SettingsRow,
  TextInput,
  StatusDot,
} from "@kosmos/visuals";
import type { PairingCode, SyncPeer, SyncSnapshot } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const snapshot = ref<SyncSnapshot | null>(null);
const pairingInput = ref("");
const pairingCode = ref<PairingCode | null | undefined>(undefined);
const pairingModalOpen = ref(false);
const copied = ref(false);
const loading = ref(true);
const refreshing = ref(false);
const disconnecting = ref(new Set<string>());
const SYNC_REFRESH_INTERVAL_MS = 8_000;
let timer: ReturnType<typeof setInterval> | null = null;

const syncStatusLabel = computed(() =>
  snapshot.value?.status === "running" ? "Работает" : snapshot.value ? "Остановлена" : "Недоступна",
);
const syncStatusTone = computed(() =>
  snapshot.value?.status === "running" ? "success" : snapshot.value ? "warning" : "danger",
);
const transportLabel = computed(
  () =>
    ({
      iroh: "Iroh",
      relay: "Relay",
      lan: "Локальная сеть",
      unknown: "Не определён",
    })[snapshot.value?.transport ?? "unknown"],
);

async function refresh() {
  if (refreshing.value) return;
  refreshing.value = true;
  const next = await props.client.call<SyncSnapshot>("getSyncSnapshot", undefined, "sync");
  if (next) snapshot.value = next;
  loading.value = false;
  refreshing.value = false;
}
async function requestPairingCode() {
  pairingCode.value = await props.client.call<PairingCode | null>(
    "getPairingCode",
    undefined,
    "pairing",
  );
  copied.value = false;
}
async function openPairingModal() {
  pairingModalOpen.value = true;
  if (snapshot.value?.own_pairing_code_available) await requestPairingCode();
}
async function copyPairingCode() {
  if (!pairingCode.value?.code) return;
  try {
    await navigator.clipboard.writeText(pairingCode.value.code);
    copied.value = true;
  } catch {
    props.client.error.value = "Не удалось скопировать код.";
  }
}
async function connect() {
  const code = pairingInput.value.trim();
  if (code.length < 8 || code.length > 256) {
    props.client.error.value = "Введите корректный код сопряжения (не менее 8 символов).";
    return;
  }
  if (await props.client.call("connectWithPairingCode", { code }, "connect")) {
    pairingInput.value = "";
    pairingModalOpen.value = false;
    await refresh();
  }
}
async function disconnect(peer: SyncPeer) {
  if (!peer.id || disconnecting.value.has(peer.id) || !window.confirm("Отключить это устройство?"))
    return;
  disconnecting.value.add(peer.id);
  const result = await props.client.call(
    "disconnectPeer",
    { peer_id: peer.id },
    `disconnect:${peer.id}`,
  );
  disconnecting.value.delete(peer.id);
  if (result) await refresh();
}
onMounted(() => {
  void refresh();
  timer = setInterval(() => void refresh(), SYNC_REFRESH_INTERVAL_MS);
});
onBeforeUnmount(() => {
  if (timer) clearInterval(timer);
});
</script>

<template>
  <section class="stack">
    <SettingsList>
      <SettingsRow title="Состояние синхронизации" :description="`Транспорт: ${transportLabel}`">
        <template #control>
          <div class="flex items-center gap-2">
            <StatusDot
              :tone="syncStatusTone"
              :label="syncStatusLabel"
            />
          </div>
        </template>
      </SettingsRow>
      <SettingsRow
        v-if="snapshot?.local_device"
        title="Локальное устройство"
        :description="snapshot.local_device.name || snapshot.local_device.id"
      />
      <SettingsButtonRow
        v-if="props.client.error.value"
        title="Менеджер недоступен"
        :description="props.client.error.value"
        button-label="Повторить"
        variant="surface"
        :disabled="refreshing"
        @click="refresh"
      />
      <SettingsButtonRow
        v-else
        title="Состояние синхронизации"
        description="Обновить данные подключения"
        button-label="Обновить"
        variant="surface"
        :disabled="refreshing"
        @click="refresh"
      />
    </SettingsList>
    <SettingsList>
      <SettingsRow title="Сопряжение" description="Код доступен только после явного действия.">
        <template #control>
          <Button
            variant="surface"
            size="sm"
            class="!h-8 !w-8 !p-0"
            aria-label="Открыть сопряжение"
            title="Открыть сопряжение"
            @click="openPairingModal"
          >
            <svg
              aria-hidden="true"
              class="size-4"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="m10 13 2-2a4 4 0 0 1 6 0l1 1a4 4 0 0 1-6 6l-1-1" />
              <path d="m14 11-2 2a4 4 0 0 1-6 0l-1-1a4 4 0 0 1 6-6l1 1" />
            </svg>
          </Button>
        </template>
      </SettingsRow>
    </SettingsList>
    <Modal :open="pairingModalOpen" title="Сопряжение устройства" @close="pairingModalOpen = false">
      <div class="stack">
        <SettingsRow
          title="Ваш код"
          :description="pairingCode?.code ?? 'Код недоступен.'"
          :muted="!pairingCode?.code"
        >
          <template #control>
            <Button
              variant="surface"
              size="sm"
              :disabled="!pairingCode?.code"
              @click="copyPairingCode"
            >
              {{ copied ? "Скопировано" : "Копировать" }}
            </Button>
          </template>
        </SettingsRow>
        <SettingsRow title="Код подключения" stacked>
          <template #control>
            <div class="flex w-full items-center gap-2">
              <TextInput
                v-model="pairingInput"
                class="min-w-0 flex-1"
                placeholder="Код сопряжения"
              />
              <Button
                :variant="pairingInput.trim() ? 'surface' : 'ghost'"
                size="sm"
                class="!h-8 !w-8 !p-0"
                :disabled="!pairingInput.trim()"
                aria-label="Подключить устройство"
                title="Подключить устройство"
                @click="connect"
              >
                <svg
                  aria-hidden="true"
                  class="size-4"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2.2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <path d="M5 12h14" />
                  <path d="m13 6 6 6-6 6" />
                </svg>
              </Button>
            </div>
          </template>
        </SettingsRow>
      </div>
    </Modal>
    <SettingsList>
      <SettingsRow
        v-for="peer in snapshot?.peers ?? []"
        :key="peer.id"
        :title="peer.name"
        :description="`${peer.status === 'online' ? 'Онлайн' : peer.status === 'offline' ? 'Оффлайн' : 'Статус неизвестен'}${peer.last_seen ? ` · Был в сети ${peer.last_seen}` : ''}`"
      >
        <template #control>
          <Button
            variant="danger"
            size="sm"
            :disabled="!snapshot || snapshot.status !== 'running' || disconnecting.has(peer.id)"
            @click="disconnect(peer)"
            >Отключить</Button
          >
        </template>
      </SettingsRow>
      <SettingsRow
        v-if="!loading && !snapshot?.peers.length"
        title="Подключённых устройств нет"
        description="Новые устройства появятся здесь после сопряжения"
      />
      <SettingsRow v-if="loading" title="Загрузка…" />
    </SettingsList>
  </section>
</template>
