<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { Button, SettingsList, SettingsRow, StatusDot, TextInput } from "@kosmos/visuals";
import type { PairingCode, SyncPeer, SyncSnapshot } from "../manager-api";
import type { ManagerClient } from "../composables/useManagerClient";

const props = defineProps<{ client: ManagerClient }>();
const snapshot = ref<SyncSnapshot | null>(null);
const pairingInput = ref("");
const pairingCode = ref<PairingCode | null | undefined>(undefined);
const copied = ref(false);
const loading = ref(true);
const refreshing = ref(false);
const disconnecting = ref(new Set<string>());
const SYNC_REFRESH_INTERVAL_MS = 8_000;
let timer: ReturnType<typeof setInterval> | null = null;

const syncStatusLabel = computed(() =>
  snapshot.value?.status === "running" ? "Работает" : snapshot.value ? "Остановлена" : "Недоступна",
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
    <div class="card">
      <div class="card-heading">
        <span>Состояние синхронизации</span
        ><StatusDot :tone="snapshot?.status === 'running' ? 'success' : 'warning'" /><strong>{{
          syncStatusLabel
        }}</strong>
      </div>
      <p class="muted">Транспорт: {{ transportLabel }}</p>
      <p v-if="snapshot?.local_device" class="muted">
        Это устройство:
        {{ snapshot.local_device.name || snapshot.local_device.id }}
      </p>
      <p v-if="props.client.error.value" class="error">
        {{ props.client.error.value }}
      </p>
      <Button variant="ghost" size="sm" :disabled="refreshing" @click="refresh"
        >Повторить запрос</Button
      >
    </div>
    <div class="card">
      <h2>Сопряжение</h2>
      <p class="muted">Код доступен только после явного действия.</p>
      <div class="toolbar">
        <Button
          size="sm"
          :disabled="
            !snapshot || snapshot.status !== 'running' || !snapshot.own_pairing_code_available
          "
          @click="requestPairingCode"
          >Показать код</Button
        >
        <code v-if="pairingCode?.code">{{ pairingCode.code }}</code>
        <Button v-if="pairingCode?.code" variant="ghost" size="sm" @click="copyPairingCode">{{
          copied ? "Скопировано" : "Копировать"
        }}</Button>
        <span v-else-if="pairingCode === null && snapshot">Код недоступен.</span>
      </div>
      <div class="toolbar">
        <TextInput
          v-model="pairingInput"
          class="toolbar-input"
          aria-label="Код сопряжения"
          maxlength="256"
          placeholder="Введите код сопряжения"
        />
        <Button
          size="sm"
          :disabled="
            !snapshot ||
            snapshot.status !== 'running' ||
            !snapshot.pairing_available ||
            !pairingInput.trim()
          "
          @click="connect"
          >Подключить</Button
        >
      </div>
    </div>
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
