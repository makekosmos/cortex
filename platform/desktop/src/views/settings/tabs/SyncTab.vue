<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { Copy, Link2 } from "@lucide/vue";
import {
  Button,
  EmptyState,
  Modal,
  SettingsContentHeader,
  SyncNodeRow,
  useToast,
} from "@kosmos/visuals";

type SyncPeerStatus = "online" | "offline";
type SyncPeerDeviceKind = "desktop" | "laptop" | "phone" | "unknown";

interface SyncPeerInfo {
  deviceId: string;
  deviceName: string;
  lastSeen: string | null;
  status: SyncPeerStatus;
  deviceKind: SyncPeerDeviceKind;
}

interface SyncStatusSnapshot {
  running: boolean;
  transport: "iroh" | "relay" | "lan" | "unknown";
  pairingAvailable: boolean;
  ownPairingCodeAvailable: boolean;
  peers: SyncPeerInfo[];
}

const toast = useToast();
const snapshot = ref<SyncStatusSnapshot | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);
const disconnecting = ref(new Set<string>());

const pairingModalOpen = ref(false);
const pairingLoading = ref(false);
const pairingCode = ref<string | null>(null);
const pairingInput = ref("");
const connectBusy = ref(false);

const peers = computed(() => snapshot.value?.peers ?? []);
const transportLabel = computed(() => {
  switch (snapshot.value?.transport) {
    case "iroh":
      return "iroh";
    case "relay":
      return "relay";
    case "lan":
      return "LAN";
    default:
      return "неизвестно";
  }
});

function inferDeviceKind(name: string): SyncPeerDeviceKind {
  const n = name.toLowerCase();
  if (/(iphone|android|phone|mobile)/.test(n)) return "phone";
  if (/(macbook|laptop|notebook)/.test(n)) return "laptop";
  if (/(pc|desktop|windows|imac)/.test(n)) return "desktop";
  return "unknown";
}

function normalizeSnapshot(raw: any): SyncStatusSnapshot {
  const peers = Array.isArray(raw?.peers)
    ? raw.peers.map((p: any) => ({
        deviceId: String(p?.deviceId ?? p?.device_id ?? ""),
        deviceName: String(p?.deviceName ?? p?.device_name ?? "Неизвестное устройство"),
        lastSeen:
          typeof p?.lastSeen === "string"
            ? p.lastSeen
            : typeof p?.last_seen === "string"
              ? p.last_seen
              : null,
        status: p?.status === "online" ? "online" : "offline",
        deviceKind:
          p?.deviceKind === "desktop" || p?.deviceKind === "laptop" || p?.deviceKind === "phone"
            ? p.deviceKind
            : inferDeviceKind(String(p?.deviceName ?? p?.device_name ?? "")),
      }))
    : [];
  return {
    running: !!raw?.running,
    transport:
      raw?.transport === "iroh" || raw?.transport === "relay" || raw?.transport === "lan"
        ? raw.transport
        : "unknown",
    pairingAvailable: !!raw?.pairingAvailable,
    ownPairingCodeAvailable: !!raw?.ownPairingCodeAvailable,
    peers,
  };
}

async function loadSnapshot() {
  loading.value = true;
  error.value = null;
  try {
    snapshot.value = normalizeSnapshot(await window.kepler.settings.sync.snapshot());
  } catch (e) {
    error.value = "Не удалось загрузить состояние синхронизации.";
  } finally {
    loading.value = false;
  }
}

async function openPairingModal() {
  pairingModalOpen.value = true;
  pairingLoading.value = true;
  pairingCode.value = null;
  try {
    pairingCode.value = await window.kepler.settings.sync.getPairingCode();
  } finally {
    pairingLoading.value = false;
  }
}

async function copyCode(code: string) {
  try {
    await navigator.clipboard.writeText(code);
  } catch {
    await window.kepler.settings.sync.copyPairingCode(code);
  }
  toast.success("Код скопирован");
}

async function disconnectPeer(deviceId: string) {
  disconnecting.value.add(deviceId);
  try {
    await window.kepler.settings.sync.disconnectPeer(deviceId);
    await loadSnapshot();
    toast.success("Устройство отключено");
  } catch {
    toast.error("Не удалось отключить устройство");
  } finally {
    disconnecting.value.delete(deviceId);
  }
}

async function connectWithCode() {
  if (pairingInput.value.trim().length < 8) {
    toast.error("Некорректный код синхронизации");
    return;
  }
  connectBusy.value = true;
  try {
    await window.kepler.settings.sync.connectWithPairingCode(pairingInput.value.trim());
    pairingInput.value = "";
    pairingModalOpen.value = false;
    await loadSnapshot();
    toast.success("Подключение отправлено");
  } catch {
    toast.error("Не удалось подключиться по коду");
  } finally {
    connectBusy.value = false;
  }
}

let unsubscribe: (() => void) | null = null;
let timer: number | null = null;

onMounted(async () => {
  await loadSnapshot();
  unsubscribe = window.kepler.settings.sync.onUpdated(() => {
    void loadSnapshot();
  });
  timer = window.setInterval(() => {
    void loadSnapshot();
  }, 8000);
});

onBeforeUnmount(() => {
  unsubscribe?.();
  if (timer) window.clearInterval(timer);
});
</script>

<template>
  <div class="advanced-page gap-4">
    <SettingsContentHeader />
    <div class="px-1">
      <div class="mb-2 text-[1.2rem] font-semibold text-[var(--foreground)]">Синхронизация</div>
      <div class="mb-4 text-[0.85rem] text-[var(--muted-foreground)]">
        Устройства, которые подключались к этому Kosmos. Транспорт:
        {{ transportLabel }}.
      </div>
      <div class="mb-4 flex gap-2">
        <Button @click="openPairingModal"
          ><template #icon><Copy :size="14" /></template>Показать код</Button
        >
      </div>

      <div v-if="loading" class="text-[var(--muted-foreground)]">Загрузка…</div>
      <div v-else-if="error" class="text-[var(--destructive)]">{{ error }}</div>
      <EmptyState
        v-else-if="!peers.length"
        title="Подключённых устройств пока нет."
        description="Когда другое устройство подключится к этому Kosmos, оно появится здесь."
      />
      <div v-else class="flex flex-col gap-2">
        <SyncNodeRow
          v-for="peer in peers"
          :key="peer.deviceId"
          :device-kind="peer.deviceKind"
          :name="peer.deviceName || 'Неизвестное устройство'"
          :last-seen-label="
            peer.status === 'online'
              ? 'Сейчас подключено'
              : peer.lastSeen
                ? `Последнее подключение: ${new Intl.DateTimeFormat('ru-RU', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(peer.lastSeen))}`
                : 'Последнее подключение неизвестно'
          "
          :status="peer.status"
          :status-label="peer.status === 'online' ? 'Онлайн' : 'Оффлайн'"
          :disconnecting="disconnecting.has(peer.deviceId)"
          @disconnect="disconnectPeer(peer.deviceId)"
        />
      </div>
    </div>

    <Modal :open="pairingModalOpen" title="Код подключения" @close="pairingModalOpen = false">
      <div class="flex flex-col gap-4">
        <div class="text-sm text-[var(--muted-foreground)]">
          Скопируй этот код на другом устройстве, чтобы подключить его к этому Kosmos.
        </div>
        <div
          v-if="pairingLoading"
          class="rounded-xl border border-[var(--border)] p-4 text-center text-[var(--muted-foreground)]"
        >
          Готовлю код подключения…
        </div>
        <template v-else>
          <button
            v-if="pairingCode"
            type="button"
            class="rounded-xl border border-[var(--border)] px-4 py-3 text-left font-mono text-sm"
            @click="copyCode(pairingCode)"
          >
            {{ pairingCode }}
          </button>
          <div
            v-else
            class="rounded-xl border border-[var(--border)] p-4 text-[var(--muted-foreground)]"
          >
            Код подключения сейчас недоступен.
          </div>
        </template>
        <div class="flex gap-2">
          <Button v-if="pairingCode" variant="ghost" @click="copyCode(pairingCode)"
            ><template #icon><Copy :size="14" /></template>Скопировать</Button
          >
          <Button variant="primary" :loading="connectBusy" @click="connectWithCode"
            ><template #icon><Link2 :size="14" /></template>Подключиться по коду</Button
          >
        </div>
        <input
          v-model="pairingInput"
          class="w-full rounded-lg border border-[var(--border)] bg-transparent px-3 py-2 text-sm text-[var(--foreground)]"
          placeholder="Введите код подключения"
        />
      </div>
    </Modal>
  </div>
</template>
