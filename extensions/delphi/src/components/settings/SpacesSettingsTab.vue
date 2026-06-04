<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, shallowRef } from "vue";
import { Globe2 } from "@lucide/vue";
import { useRouter } from "vue-router";
import QRCode from "qrcode";
import { Button, EmptyState, Modal, SettingsList, SettingsRow, TextInput } from "@kosmos/visuals";
import {
  type Space,
  deriveSpaceId,
  formatSpaceCode,
  getActiveSpace,
  getSpaces,
  removeSpace,
  renameSpace,
} from "@/services/space/space-manager";
import {
  activeSpaceCode as sharedActiveSpaceCode,
  connectedPeerCount,
  connectedPeerNames,
  connectionState,
  requestLeaveSpace,
} from "@/composables/useSyncState";

const spaces = ref<Space[]>([]);
const activeSpaceCode = shallowRef<string | null>(null);
const renamingCode = shallowRef<string | null>(null);
const renameInput = shallowRef("");
const deletingCode = shallowRef<string | null>(null);

const router = useRouter();
const isElectron = typeof window !== "undefined" && !!window.electronAPI;
const showQrOverlay = shallowRef(false);
const fullQrDataUrl = shallowRef("");
const qrOverlayPayload = shallowRef("");
const qrLinkCopied = shallowRef(false);

const activeP2pCode = computed(() => sharedActiveSpaceCode.value ?? activeSpaceCode.value);

const connectionLabel = computed(() => {
  switch (connectionState.value) {
    case "online":
      return connectedPeerCount.value > 0
        ? `${connectedPeerCount.value} ${connectedPeerCount.value === 1 ? "пир" : "пиров"} в сети`
        : "Соединение установлено";
    case "syncing":
      return "Соединение в процессе";
    case "offline":
    default:
      return "Пиры не найдены";
  }
});

const connectionToneClass = computed(() => `p2p-dot p2p-dot--${connectionState.value}`);

async function openQrOverlay() {
  const code = activeP2pCode.value;
  if (!code) return;
  try {
    let payload = "";
    if (isElectron && window.electronAPI?.invoke) {
      payload = ((await window.electronAPI.invoke("sync:getQrPayload", code)) as string) || "";
    }
    if (!payload) payload = formatSpaceCode(code);
    qrOverlayPayload.value = payload;
    fullQrDataUrl.value = await QRCode.toDataURL(payload, {
      width: 512,
      margin: 3,
      color: { dark: "black", light: "white" },
      errorCorrectionLevel: "M",
    });
  } catch {
    fullQrDataUrl.value = "";
  }
  showQrOverlay.value = true;
}

async function copyQrLink() {
  if (!qrOverlayPayload.value) return;
  try {
    await navigator.clipboard.writeText(qrOverlayPayload.value);
    qrLinkCopied.value = true;
    setTimeout(() => {
      qrLinkCopied.value = false;
    }, 2000);
  } catch {
    /* clipboard blocked */
  }
}

function leaveSpace() {
  requestLeaveSpace();
  void router.push("/");
}

let peerPollHandle: ReturnType<typeof setInterval> | null = null;

function refreshPeerStatus() {
  if (!isElectron || !window.electronAPI?.invoke) return;
  window.electronAPI
    .invoke("lan-sync:getStatus")
    .then((status: { active: boolean; peers: number; peerNames?: string[] }) => {
      connectedPeerCount.value = status.peers;
      connectedPeerNames.value = status.peerNames ?? [];
      connectionState.value = status.peers > 0 ? "online" : "offline";
    })
    .catch(() => {});
}

async function loadSpacesState() {
  spaces.value = await getSpaces();
  activeSpaceCode.value = await getActiveSpace();
}

function startRename(space: Space) {
  renamingCode.value = space.code;
  renameInput.value = space.name;
}

async function confirmRename() {
  if (!renamingCode.value) return;

  await renameSpace(renamingCode.value, renameInput.value);
  await loadSpacesState();
  renamingCode.value = null;
}

function cancelRename() {
  renamingCode.value = null;
  renameInput.value = "";
}

async function confirmDeleteSpace() {
  if (!deletingCode.value) return;

  const code = deletingCode.value;
  if (window.electronAPI?.invoke) {
    const spaceId = await deriveSpaceId(code);
    await window.electronAPI.invoke("db:deleteSpace", spaceId).catch(() => {});
  }

  await removeSpace(code);
  await loadSpacesState();
  deletingCode.value = null;
}

onMounted(() => {
  void loadSpacesState();
  refreshPeerStatus();
  peerPollHandle = setInterval(refreshPeerStatus, 3000);
});

onUnmounted(() => {
  if (peerPollHandle) clearInterval(peerPollHandle);
});
</script>

<template>
  <div class="settings-tab" data-testid="delphi-settings-spaces-tab">
    <header class="settings-tab-header">
      <p class="settings-tab-kicker">Настройки Delphi</p>
      <h1 class="settings-tab-title">Пространства</h1>
      <p class="settings-tab-subtitle">
        Управление пространствами синхронизации и их локальными данными.
      </p>
    </header>

    <SettingsList v-if="isElectron && activeP2pCode">
      <SettingsRow
        title="P2P синхронизация"
        description="Подключение устройств в текущем пространстве."
      >
        <template #control>
          <span class="p2p-status">
            <span :class="connectionToneClass" />
            <span class="p2p-status-text">{{ connectionLabel }}</span>
          </span>
        </template>
      </SettingsRow>

      <SettingsRow title="Активное пространство" :description="formatSpaceCode(activeP2pCode)">
        <template #control>
          <Button type="button" size="sm" variant="ghost" @click="openQrOverlay">
            Показать QR-код
          </Button>
        </template>
      </SettingsRow>

      <SettingsRow
        v-if="connectedPeerNames.length > 0"
        title="Подключённые пиры"
        :description="connectedPeerNames.join(', ')"
      />

      <SettingsRow title="Выход из пространства" description="Отключить текущую P2P-сессию.">
        <template #control>
          <Button type="button" size="sm" variant="danger" @click="leaveSpace"> Покинуть </Button>
        </template>
      </SettingsRow>
    </SettingsList>

    <SettingsList>
      <SettingsRow title="Список пространств" description="Локальные legacy-пространства Delphi.">
        <template #control>
          <Globe2 :size="16" class="settings-row-icon" />
        </template>
      </SettingsRow>

      <div v-if="spaces.length === 0" class="settings-list-empty">
        <EmptyState compact title="Сохранённых пространств пока нет" />
      </div>

      <template v-else>
        <SettingsRow
          v-for="space in spaces"
          :key="space.code"
          :title="deletingCode === space.code ? 'Удалить пространство?' : space.name"
          :description="
            deletingCode === space.code
              ? 'Будут удалены локальные данные этого пространства.'
              : activeSpaceCode === space.code
                ? `Активно · ${formatSpaceCode(space.code)}`
                : formatSpaceCode(space.code)
          "
        >
          <template #control>
            <div v-if="deletingCode === space.code" class="settings-space-actions">
              <Button type="button" size="sm" variant="danger" @click="confirmDeleteSpace">
                Удалить
              </Button>
              <Button type="button" size="sm" variant="ghost" @click="deletingCode = null">
                Отмена
              </Button>
            </div>

            <div v-else-if="renamingCode === space.code" class="settings-space-rename">
              <TextInput
                v-model="renameInput"
                size="sm"
                autofocus
                @keydown.enter="confirmRename"
                @keydown.escape="cancelRename"
              />
              <Button type="button" size="sm" variant="primary" @click="confirmRename">
                Сохранить
              </Button>
              <Button type="button" size="sm" variant="ghost" @click="cancelRename">
                Отмена
              </Button>
            </div>

            <div v-else class="settings-space-actions">
              <Button type="button" size="sm" variant="ghost" @click="startRename(space)">
                Переименовать
              </Button>
              <Button
                v-if="activeSpaceCode !== space.code"
                type="button"
                size="sm"
                variant="danger"
                @click="deletingCode = space.code"
              >
                Удалить
              </Button>
            </div>
          </template>
        </SettingsRow>
      </template>
    </SettingsList>

    <Modal
      :open="showQrOverlay"
      title="Пространство"
      width="min(420px, 92vw)"
      @close="showQrOverlay = false"
    >
      <div class="p2p-qr-content">
        <p v-if="activeP2pCode" class="p2p-qr-code font-[var(--font-mono)]">
          {{ formatSpaceCode(activeP2pCode) }}
        </p>
        <img
          v-if="fullQrDataUrl"
          :src="fullQrDataUrl"
          alt="QR"
          class="p2p-qr-image"
          width="320"
          height="320"
        />
        <p class="p2p-qr-hint">Отсканируйте QR или вставьте ссылку на другом устройстве</p>
      </div>
      <template #footer>
        <Button v-if="qrOverlayPayload" type="button" variant="primary" @click="copyQrLink">
          {{ qrLinkCopied ? "Скопировано!" : "Скопировать ссылку" }}
        </Button>
        <Button type="button" variant="ghost" @click="showQrOverlay = false">Закрыть</Button>
      </template>
    </Modal>
  </div>
</template>

<style scoped>
.settings-tab {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.settings-tab-header {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.settings-tab-kicker {
  font-size: var(--kosmos-text-caption-size);
  font-weight: 700;
  letter-spacing: 0;
  text-transform: uppercase;
  color: var(--muted-foreground);
}

.settings-tab-title {
  font-size: var(--kosmos-text-page-title-size);
  line-height: var(--kosmos-text-page-title-line-height);
  font-weight: var(--kosmos-text-page-title-weight);
  color: var(--foreground);
}

.settings-tab-subtitle {
  max-width: 42rem;
  font-size: var(--kosmos-text-body-size);
  color: var(--muted-foreground);
}

.settings-space-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.55rem;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.p2p-status {
  display: inline-flex;
  align-items: center;
  gap: 0.55rem;
  font-size: 0.85rem;
  color: var(--muted-foreground);
}

.p2p-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: currentColor;
  flex-shrink: 0;
}

.p2p-dot--online {
  color: var(--status-success);
}

.p2p-dot--syncing {
  color: var(--status-warning);
}

.p2p-dot--offline {
  color: var(--destructive);
}

.p2p-status-text {
  color: var(--foreground);
}

.settings-row-icon {
  color: var(--muted-foreground);
}

.settings-list-empty {
  padding: 1rem;
}

.settings-space-rename {
  display: inline-flex;
  align-items: center;
  gap: 0.55rem;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.p2p-qr-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}

.p2p-qr-code {
  font-size: 1.4rem;
  font-weight: 700;
  letter-spacing: 0.16em;
  color: var(--foreground);
}

.p2p-qr-image {
  border-radius: 0.6rem;
}

.p2p-qr-hint {
  max-width: 22rem;
  font-size: 0.78rem;
  text-align: center;
  color: var(--muted-foreground);
}
</style>
