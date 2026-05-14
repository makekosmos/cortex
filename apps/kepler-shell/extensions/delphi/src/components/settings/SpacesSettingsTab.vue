<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, shallowRef } from "vue";
import { Globe2, Radio } from "lucide-vue-next";
import { useRouter } from "vue-router";
import QRCode from "qrcode";
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

const activeP2pCode = computed(
  () => sharedActiveSpaceCode.value ?? activeSpaceCode.value,
);

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

const connectionToneClass = computed(
  () => `p2p-dot p2p-dot--${connectionState.value}`,
);

async function openQrOverlay() {
  const code = activeP2pCode.value;
  if (!code) return;
  try {
    let payload = "";
    if (isElectron && window.electronAPI?.invoke) {
      payload =
        ((await window.electronAPI.invoke(
          "sync:getQrPayload",
          code,
        )) as string) || "";
    }
    if (!payload) payload = formatSpaceCode(code);
    qrOverlayPayload.value = payload;
    fullQrDataUrl.value = await QRCode.toDataURL(payload, {
      width: 512,
      margin: 3,
      color: { dark: "#000000", light: "#ffffff" },
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
    .then(
      (status: { active: boolean; peers: number; peerNames?: string[] }) => {
        connectedPeerCount.value = status.peers;
        connectedPeerNames.value = status.peerNames ?? [];
        connectionState.value = status.peers > 0 ? "online" : "offline";
      },
    )
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

    <section v-if="isElectron && activeP2pCode" class="settings-card">
      <div class="settings-card-header">
        <div class="settings-card-icon">
          <Radio :size="16" />
        </div>
        <div>
          <h2>P2P синхронизация</h2>
          <p>Подключение устройств в текущем пространстве.</p>
        </div>
      </div>

      <div class="p2p-body">
        <div class="p2p-status">
          <span :class="connectionToneClass" />
          <span class="p2p-status-text">{{ connectionLabel }}</span>
        </div>

        <div class="p2p-code-row">
          <div>
            <p class="p2p-label">Активное пространство</p>
            <p class="p2p-code">{{ formatSpaceCode(activeP2pCode) }}</p>
          </div>
          <button
            type="button"
            class="settings-action-button"
            @click="openQrOverlay"
          >
            Показать QR-код
          </button>
        </div>

        <div v-if="connectedPeerNames.length > 0" class="p2p-peers">
          <p class="p2p-label">Подключённые пиры</p>
          <ul class="p2p-peer-list">
            <li v-for="name in connectedPeerNames" :key="name">{{ name }}</li>
          </ul>
        </div>

        <div class="p2p-leave">
          <button
            type="button"
            class="settings-action-button settings-action-button--danger"
            @click="leaveSpace"
          >
            Покинуть пространство
          </button>
        </div>
      </div>
    </section>

    <section class="settings-card">
      <div class="settings-card-header">
        <div class="settings-card-icon">
          <Globe2 :size="16" />
        </div>
        <div>
          <h2>Список пространств</h2>
          <p>Переименовывайте и очищайте неактивные пространства.</p>
        </div>
      </div>

      <div v-if="spaces.length === 0" class="settings-empty">
        Сохранённых пространств пока нет.
      </div>

      <div v-else class="settings-space-list">
        <article
          v-for="space in spaces"
          :key="space.code"
          :class="[
            'settings-space-card',
            activeSpaceCode === space.code ? 'settings-space-card--active' : '',
          ]"
        >
          <template v-if="deletingCode === space.code">
            <div class="settings-space-delete">
              <div>
                <h3>Удалить пространство?</h3>
                <p>Будут удалены локальные данные этого пространства.</p>
              </div>

              <div class="settings-space-actions">
                <button
                  type="button"
                  class="settings-action-button settings-action-button--danger"
                  @click="confirmDeleteSpace"
                >
                  Да, удалить
                </button>
                <button
                  type="button"
                  class="settings-action-button"
                  @click="deletingCode = null"
                >
                  Отмена
                </button>
              </div>
            </div>
          </template>

          <template v-else-if="renamingCode === space.code">
            <div class="settings-space-rename">
              <input
                v-model="renameInput"
                class="settings-text-input"
                autofocus
                @keyup.enter="confirmRename"
                @keyup.escape="cancelRename"
              />

              <div class="settings-space-actions">
                <button
                  type="button"
                  class="settings-action-button settings-action-button--primary"
                  @click="confirmRename"
                >
                  Сохранить
                </button>
                <button
                  type="button"
                  class="settings-action-button"
                  @click="cancelRename"
                >
                  Отмена
                </button>
              </div>
            </div>
          </template>

          <template v-else>
            <div class="settings-space-main">
              <div>
                <div class="settings-space-title-row">
                  <h3 class="settings-space-title">{{ space.name }}</h3>
                  <span
                    v-if="activeSpaceCode === space.code"
                    class="settings-space-badge"
                  >
                    Активно
                  </span>
                </div>

                <p class="settings-space-code">
                  {{ formatSpaceCode(space.code) }}
                </p>
              </div>

              <div class="settings-space-actions">
                <button
                  type="button"
                  class="settings-action-button"
                  @click="startRename(space)"
                >
                  Переименовать
                </button>
                <button
                  v-if="activeSpaceCode !== space.code"
                  type="button"
                  class="settings-action-button settings-action-button--danger"
                  @click="deletingCode = space.code"
                >
                  Удалить
                </button>
              </div>
            </div>
          </template>
        </article>
      </div>
    </section>

    <div
      v-if="showQrOverlay"
      class="p2p-qr-overlay"
      @click.self="showQrOverlay = false"
    >
      <div class="p2p-qr-dialog">
        <p class="p2p-qr-kicker">Пространство</p>
        <p v-if="activeP2pCode" class="p2p-qr-code">
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
        <p class="p2p-qr-hint">
          Отсканируйте QR или вставьте ссылку на другом устройстве
        </p>
        <div class="p2p-qr-actions">
          <button
            v-if="qrOverlayPayload"
            type="button"
            class="settings-action-button settings-action-button--primary"
            @click="copyQrLink"
          >
            {{ qrLinkCopied ? "Скопировано!" : "Скопировать ссылку" }}
          </button>
          <button
            type="button"
            class="settings-action-button"
            @click="showQrOverlay = false"
          >
            Закрыть
          </button>
        </div>
      </div>
    </div>
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
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--muted-foreground);
}

.settings-tab-title {
  font-size: clamp(1.8rem, 2.6vw, 2.4rem);
  line-height: 1;
  font-weight: 700;
  color: var(--foreground);
}

.settings-tab-subtitle {
  max-width: 42rem;
  font-size: 0.95rem;
  color: var(--muted-foreground);
}

.settings-card {
  border: 1px solid var(--border);
  border-radius: 1.25rem;
  background: color-mix(in srgb, var(--background) 86%, var(--secondary));
  overflow: hidden;
}

.settings-card-header {
  display: flex;
  align-items: center;
  gap: 0.9rem;
  padding: 1.2rem 1.25rem 1rem;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
}

.settings-card-header h2 {
  font-size: 1rem;
  font-weight: 600;
  color: var(--foreground);
}

.settings-card-header p {
  margin-top: 0.15rem;
  font-size: 0.85rem;
  color: var(--muted-foreground);
}

.settings-card-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 2rem;
  height: 2rem;
  border-radius: 0.75rem;
  background: color-mix(in srgb, var(--secondary) 82%, transparent);
  color: var(--foreground);
  flex-shrink: 0;
}

.settings-empty {
  padding: 1rem 1.25rem 1.25rem;
  color: var(--muted-foreground);
  font-size: 0.9rem;
}

.settings-space-list {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
  padding: 1rem 1.25rem 1.25rem;
}

.settings-space-card {
  border: 1px solid color-mix(in srgb, var(--border) 78%, transparent);
  border-radius: 1rem;
  background: color-mix(in srgb, var(--background) 92%, transparent);
}

.settings-space-card--active {
  border-color: color-mix(in srgb, rgb(16 185 129) 40%, var(--border));
  background: color-mix(in srgb, rgb(16 185 129) 8%, var(--background));
}

.settings-space-main,
.settings-space-rename,
.settings-space-delete {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem;
}

.settings-space-rename,
.settings-space-delete {
  align-items: stretch;
}

.settings-space-title-row {
  display: flex;
  align-items: center;
  gap: 0.55rem;
}

.settings-space-title {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--foreground);
}

.settings-space-code {
  margin-top: 0.25rem;
  font-family: var(--font-mono, monospace);
  font-size: 0.78rem;
  letter-spacing: 0.12em;
  color: var(--muted-foreground);
}

.settings-space-badge {
  display: inline-flex;
  align-items: center;
  border-radius: 999px;
  padding: 0.2rem 0.5rem;
  background: color-mix(in srgb, rgb(16 185 129) 16%, transparent);
  color: rgb(16 185 129);
  font-size: 0.72rem;
  font-weight: 600;
}

.settings-space-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.55rem;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.settings-action-button {
  padding: 0.6rem 0.85rem;
  border: 1px solid color-mix(in srgb, var(--border) 82%, transparent);
  border-radius: 0.85rem;
  background: color-mix(in srgb, var(--background) 96%, transparent);
  color: var(--foreground);
  font-size: 0.82rem;
  font-weight: 500;
  transition:
    border-color 120ms ease,
    background-color 120ms ease;
}

.settings-action-button:hover {
  border-color: color-mix(in srgb, var(--foreground) 20%, var(--border));
  background: color-mix(in srgb, var(--secondary) 55%, var(--background));
}

.settings-action-button--primary {
  background: color-mix(in srgb, var(--foreground) 92%, transparent);
  border-color: color-mix(in srgb, var(--foreground) 85%, transparent);
  color: var(--background);
}

.settings-action-button--primary:hover {
  background: var(--foreground);
}

.settings-action-button--danger {
  color: rgb(248 113 113);
  border-color: color-mix(in srgb, rgb(248 113 113) 26%, var(--border));
}

.settings-action-button--danger:hover {
  background: color-mix(in srgb, rgb(248 113 113) 10%, transparent);
}

.settings-text-input {
  flex: 1;
  min-width: 0;
  padding: 0.7rem 0.85rem;
  border: 1px solid color-mix(in srgb, var(--border) 82%, transparent);
  border-radius: 0.85rem;
  background: color-mix(in srgb, var(--background) 96%, transparent);
  color: var(--foreground);
  outline: none;
}

.settings-text-input:focus {
  border-color: color-mix(in srgb, var(--foreground) 22%, var(--border));
}

.settings-space-delete h3 {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--foreground);
}

.settings-space-delete p {
  margin-top: 0.25rem;
  font-size: 0.82rem;
  color: var(--muted-foreground);
}

@media (max-width: 720px) {
  .settings-space-main,
  .settings-space-rename,
  .settings-space-delete {
    flex-direction: column;
    align-items: stretch;
  }

  .settings-space-actions {
    justify-content: stretch;
  }

  .settings-action-button {
    width: 100%;
  }
}

/* --- P2P card --- */
.p2p-body {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
  padding: 1rem 1.25rem 1.25rem;
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
  color: oklch(0.75 0.14 75);
}

.p2p-dot--offline {
  color: var(--destructive);
}

.p2p-status-text {
  color: var(--foreground);
}

.p2p-code-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.85rem 1rem;
  border: 1px solid color-mix(in srgb, var(--border) 78%, transparent);
  border-radius: 0.85rem;
  background: color-mix(in srgb, var(--background) 92%, transparent);
}

.p2p-label {
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--muted-foreground);
}

.p2p-code {
  margin-top: 0.3rem;
  font-family: var(--font-mono, monospace);
  font-size: 0.95rem;
  font-weight: 600;
  letter-spacing: 0.12em;
  color: var(--foreground);
  word-break: break-all;
}

.p2p-peers {
  border-top: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
  padding-top: 0.85rem;
}

.p2p-peer-list {
  margin-top: 0.45rem;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  font-size: 0.85rem;
  color: var(--foreground);
}

.p2p-leave {
  display: flex;
  justify-content: flex-end;
}

/* --- Fullscreen QR overlay --- */
.p2p-qr-overlay {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2rem;
  background: rgba(0, 0, 0, 0.7);
}

.p2p-qr-dialog {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
  padding: 2rem;
  background: var(--background);
  border-radius: 1.25rem;
  box-shadow: 0 24px 64px rgba(0, 0, 0, 0.4);
}

.p2p-qr-kicker {
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--muted-foreground);
}

.p2p-qr-code {
  font-family: var(--font-mono, monospace);
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

.p2p-qr-actions {
  display: flex;
  gap: 0.6rem;
  width: 100%;
  margin-top: 0.4rem;
}

.p2p-qr-actions .settings-action-button {
  flex: 1;
}
</style>
