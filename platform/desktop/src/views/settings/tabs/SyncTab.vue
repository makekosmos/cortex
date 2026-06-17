<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, computed } from "vue";
import { Copy, Link2, Monitor } from "@lucide/vue";
import {
  Button,
  EmptyState,
  Modal,
  SettingsList,
  SettingsRow,
  SyncNodeRow,
  TextInput,
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
  localDevice: { deviceId: string; deviceName: string } | null;
}

const toast = useToast();
const snapshot = ref<SyncStatusSnapshot | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);
const disconnecting = ref(new Set<string>());

const pairingModalOpen = ref(false);
const pairingLoading = ref(false);
const pairingCode = ref<string | null>(null);
const pairingError = ref(false);
const pairingInput = ref("");
const connectBusy = ref(false);

const peers = computed(() => snapshot.value?.peers ?? []);
const localDevice = computed(() => snapshot.value?.localDevice ?? null);

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
  const ld = raw?.localDevice ?? raw?.local_device;
  const localDevice =
    ld && typeof ld === "object"
      ? {
          deviceId: String(ld.deviceId ?? ld.device_id ?? ""),
          deviceName: String(ld.deviceName ?? ld.device_name ?? ""),
        }
      : null;
  return {
    running: !!raw?.running,
    transport:
      raw?.transport === "iroh" || raw?.transport === "relay" || raw?.transport === "lan"
        ? raw.transport
        : "unknown",
    pairingAvailable: !!raw?.pairingAvailable,
    ownPairingCodeAvailable: !!raw?.ownPairingCodeAvailable,
    peers,
    localDevice,
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

async function loadPairingCode() {
  pairingLoading.value = true;
  pairingCode.value = null;
  pairingError.value = false;
  try {
    pairingCode.value = await window.kepler.settings.sync.getPairingCode();
  } catch {
    pairingError.value = true;
  } finally {
    pairingLoading.value = false;
  }
}

async function openPairingModal() {
  pairingModalOpen.value = true;
  // Если код уже загружен (нет ошибки) — открываем без повторной загрузки
  if (pairingCode.value !== null && !pairingError.value) return;
  await loadPairingCode();
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
  // Префетч кода подключения сразу после загрузки снапшота (O(1) чтение кэша)
  if (snapshot.value?.running) {
    void loadPairingCode();
  }
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
  <div class="security-page kosmos-scroll">
    <!-- Секция: Это устройство -->
    <template v-if="localDevice">
      <div class="ext-section-title" style="padding-left: 0">Это устройство</div>
      <div class="mb-4">
        <div
          class="flex items-center gap-3 rounded-lg border border-[var(--border)] bg-[var(--settings-list-background)] px-4 py-3"
        >
          <div
            class="flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--settings-list-background)] text-[var(--foreground)]"
          >
            <Monitor :size="18" :stroke-width="2" />
          </div>
          <div class="min-w-0 flex-1">
            <div
              class="truncate font-[var(--font-sans)] text-[length:var(--kosmos-text-body-size)] font-medium text-[var(--foreground)]"
            >
              {{ localDevice.deviceName || "Это устройство" }}
            </div>
            <div
              class="mt-0.5 font-[var(--font-mono)] text-[length:var(--kosmos-text-caption-size)] text-[var(--muted-foreground)]"
            >
              Это устройство
            </div>
          </div>
        </div>
      </div>
    </template>

    <!-- Секция: Код подключения -->
    <SettingsList class="mb-4">
      <SettingsRow title="Код подключения">
        <template #control>
          <Button variant="ghost" size="sm" @click="openPairingModal">
            <template #icon><Link2 :size="14" /></template>
            Подключить устройство
          </Button>
        </template>
      </SettingsRow>
    </SettingsList>

    <!-- Секция: Другие устройства -->
    <div class="ext-section-title" style="padding-left: 0">Другие устройства</div>

    <div v-if="loading" class="text-[var(--muted-foreground)] text-[0.85rem] py-2">Загрузка…</div>
    <div v-else-if="error" class="text-[var(--destructive)] text-[0.85rem] py-2">{{ error }}</div>
    <EmptyState
      v-else-if="!peers.length"
      title="Других устройств пока нет."
      description="Когда другое устройство подключится к этому Kosmos, оно появится здесь."
    />
    <div v-else class="flex flex-col gap-2">
      <SyncNodeRow
        v-for="peer in peers"
        :key="peer.deviceId"
        :device-kind="peer.deviceKind"
        :name="peer.deviceName || 'Неизвестное устройство'"
        :last-seen-label="peer.status === 'online' ? 'Сейчас подключено' : 'Был в сети'"
        :status="peer.status"
        :status-label="peer.status === 'online' ? 'Онлайн' : 'Оффлайн'"
        :disconnecting="disconnecting.has(peer.deviceId)"
        @disconnect="disconnectPeer(peer.deviceId)"
      />
    </div>

    <Modal :open="pairingModalOpen" title="Подключить устройство" @close="pairingModalOpen = false">
      <div class="flex flex-col gap-5">
        <!-- Секция A: Поделиться кодом -->
        <div>
          <div class="pairing-section-title">Поделиться кодом</div>
          <div class="text-[0.8125rem] text-[var(--muted-foreground)] mb-3">
            Откройте этот код на другом устройстве, чтобы подключить его к этому Kosmos.
          </div>

          <!-- loading -->
          <div v-if="pairingLoading" class="pairing-code-loading">
            <span class="pairing-spinner" aria-hidden="true" />
            <span>Готовлю код подключения…</span>
          </div>

          <!-- error -->
          <div v-else-if="pairingError" class="pairing-code-error">
            <span class="text-[0.8125rem] text-[var(--muted-foreground)]">
              Не удалось получить код подключения.
            </span>
            <button type="button" class="pairing-retry-btn" @click="loadPairingCode">
              Повторить
            </button>
          </div>

          <!-- success -->
          <button
            v-else-if="pairingCode"
            type="button"
            class="pairing-code-field"
            :title="pairingCode"
            @click="copyCode(pairingCode)"
          >
            <span class="pairing-code-text">{{ pairingCode }}</span>
            <Copy :size="13" class="pairing-code-copy-icon" aria-hidden="true" />
          </button>

          <!-- null без ошибки — sync ещё не запущен -->
          <div
            v-else-if="pairingCode === null && !pairingLoading && !pairingError"
            class="text-[0.8125rem] text-[var(--muted-foreground)] py-2"
          >
            Код подключения появится, когда синхронизация запустится.
          </div>
        </div>

        <!-- Разделитель -->
        <div class="border-t border-[var(--border)]" />

        <!-- Секция B: Подключиться по коду -->
        <div>
          <div class="pairing-section-title">Подключиться по коду</div>
          <div class="text-[0.8125rem] text-[var(--muted-foreground)] mb-3">
            Вставьте код с другого устройства.
          </div>
          <div class="flex flex-col gap-2">
            <TextInput v-model="pairingInput" placeholder="Введите код подключения" />
            <div class="flex justify-end">
              <Button variant="primary" :loading="connectBusy" @click="connectWithCode">
                <template #icon><Link2 :size="14" /></template>
                Подключиться
              </Button>
            </div>
          </div>
        </div>
      </div>
    </Modal>
  </div>
</template>

<style scoped>
.pairing-section-title {
  font-size: 0.6875rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--muted-foreground);
  padding-bottom: 6px;
  margin-bottom: 2px;
}

/* Loading state */
.pairing-code-loading {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.8125rem;
  color: var(--muted-foreground);
  padding: 10px 0;
}

.pairing-spinner {
  display: inline-block;
  width: 14px;
  height: 14px;
  border: 2px solid color-mix(in srgb, var(--foreground) 15%, transparent);
  border-top-color: color-mix(in srgb, var(--foreground) 55%, transparent);
  border-radius: 50%;
  animation: pairing-spin 0.7s linear infinite;
  flex-shrink: 0;
}

@keyframes pairing-spin {
  to {
    transform: rotate(360deg);
  }
}

/* Error state */
.pairing-code-error {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 0;
}

.pairing-retry-btn {
  font: inherit;
  font-size: 0.8125rem;
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
  background: transparent;
  border: none;
  padding: 0;
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 2px;
}

.pairing-retry-btn:hover {
  color: var(--foreground);
}

/* Code field */
.pairing-code-field {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  box-sizing: border-box;
  padding: 10px 12px;
  border-radius: 8px;
  border: none;
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  cursor: pointer;
  text-align: left;
  transition: background 120ms ease;
}

.pairing-code-field:hover {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
}

.pairing-code-text {
  flex: 1;
  min-width: 0;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.8125rem;
  color: color-mix(in srgb, var(--foreground) 85%, transparent);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pairing-code-copy-icon {
  flex-shrink: 0;
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
}

.pairing-code-field:hover .pairing-code-copy-icon {
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
}
</style>
