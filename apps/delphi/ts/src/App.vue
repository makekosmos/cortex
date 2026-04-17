<script setup lang="ts">
/* eslint-disable no-console */
import { computed, onMounted, onUnmounted, shallowRef } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import { Loader, PanelLeft, Wifi, WifiOff } from "lucide-vue-next";
import QRCode from "qrcode";
import {
  fetchProjectsFromArk,
  fetchTasksFromArk,
  getArkApiKey,
  getArkUrl,
  setArkApiKey,
  setArkUrl,
} from "@/services/sync/ark-types";
import { parseConnectionString } from "@/services/sync/pairing";
import { isLocalDbAvailable, loadAllFromLocalDb } from "@/services/storage/local-db";
import {
  deriveSpaceId,
  formatSpaceCode,
  getActiveSpace,
  getSpaces,
  saveSpace,
  setActiveSpace,
} from "@/services/space/space-manager";
import type { SyncEntity } from "@/services/sync/lan-protocol";
import { useTodoStore } from "@/store/todos";
import SideBar from "@/components/SideBar.vue";
import QuickEntry from "@/components/QuickEntry.vue";
import QuickSearch from "@/components/QuickSearch.vue";
import AuthOverlay from "@/components/AuthOverlay.vue";
import SpaceSetup from "@/components/SpaceSetup.vue";
import {
  CustomCaret,
  DesktopChrome,
  DesktopContentSurface,
  StatusDot,
  TitlebarHistoryControls,
  type StatusDotTone,
  type TitlebarPlatform,
} from "@kepler/visuals";
import { setSidebarHidden, useSidebarState } from "@/composables/useSidebarState";

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

const store = useTodoStore();
const route = useRoute();
const router = useRouter();

const quickSearchOpen = shallowRef(false);
const { sidebarHidden } = useSidebarState();

type RouterHistoryStateLike = {
  back?: string | null;
  forward?: string | null;
};

function handleGlobalKeydown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.code === "KeyK") {
    e.preventDefault();
    quickSearchOpen.value = !quickSearchOpen.value;
  }
}

// ---------------------------------------------------------------------------
// Connection state
// ---------------------------------------------------------------------------

type ConnectionState = "online" | "syncing" | "offline";
const connectionState = shallowRef<ConnectionState>("offline");
const authRequired = shallowRef(false);
const authBusy = shallowRef(false);
const authError = shallowRef<string | null>(null);

// Ark Space state (Electron P2P mode)
const isElectron = typeof window !== "undefined" && !!window.electronAPI;
const spaceRequired = shallowRef(false);
const activeSpaceCode = shallowRef<string | null>(null);
const connectedPeerCount = shallowRef(0);
const connectedPeerNames = shallowRef<string[]>([]);

const showQrOverlay = shallowRef(false);
const fullQrDataUrl = shallowRef("");
const qrOverlayPayload = shallowRef("");
const qrLinkCopied = shallowRef(false);

// Generate large QR on demand
async function openQrOverlay() {
  const code = activeSpaceCode.value;
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

const connectionIconBg = computed(() => {
  switch (connectionState.value) {
    case "online":
      return "bg-emerald-500/15";
    case "syncing":
      return "bg-amber-500/15";
    case "offline":
      return "bg-rose-500/15";
  }
});

const connectionIconColor = computed(() => {
  switch (connectionState.value) {
    case "online":
      return "text-emerald-500";
    case "syncing":
      return "text-amber-500";
    case "offline":
      return "text-rose-500";
  }
});

const connectionIcon = computed(() => {
  switch (connectionState.value) {
    case "online":
      return Wifi;
    case "syncing":
      return Loader;
    case "offline":
      return WifiOff;
  }
});

const connectionSubtext = computed(() => {
  if (connectionState.value === "online" && connectedPeerCount.value > 0) {
    return `${connectedPeerCount.value} ${connectedPeerCount.value === 1 ? "пир" : "пиров"} подключено`;
  }
  switch (connectionState.value) {
    case "online":
      return "Соединение установлено";
    case "syncing":
      return "Соединение в процессе";
    case "offline":
      return "Соединение отсутствует";
  }
});

const connectionStatusTone = computed<StatusDotTone>(() => {
  switch (connectionState.value) {
    case "online":
      return "success";
    case "syncing":
      return "warning";
    case "offline":
    default:
      return "danger";
  }
});

const connectionStatusLabel = computed(() => {
  switch (connectionState.value) {
    case "online":
      return "P2P соединение активно";
    case "syncing":
      return "P2P соединение синхронизируется";
    case "offline":
    default:
      return "P2P соединение недоступно";
  }
});

const chromePlatform = computed<TitlebarPlatform>(() => {
  if (navigator.platform.startsWith("Mac")) return "mac";
  if (navigator.platform.startsWith("Linux")) return "linux";
  return "windows";
});

function toggleSidebar() {
  setSidebarHidden(!sidebarHidden.value);
}

const historyState = computed(
  () => (router.options.history.state as RouterHistoryStateLike | undefined) ?? undefined,
);

const canGoBack = computed(() => {
  void route.fullPath;
  return Boolean(historyState.value?.back);
});

const canGoForward = computed(() => {
  void route.fullPath;
  return Boolean(historyState.value?.forward);
});

function navigateBack() {
  if (!canGoBack.value) return;
  router.back();
}

function navigateForward() {
  if (!canGoForward.value) return;
  router.forward();
}

// ---------------------------------------------------------------------------
// Cleanup registry
// ---------------------------------------------------------------------------

let cleanupLanSyncListener: (() => void) | null = null;

async function bootstrapWeb() {
  const url = getArkUrl();
  const key = getArkApiKey();

  if (!url || !key) {
    authRequired.value = true;
    authError.value = null;
    connectionState.value = "offline";
    store.setHydrated(true);
    return;
  }

  connectionState.value = "syncing";
  authRequired.value = false;
  authError.value = null;
  store.setHydrated(false);

  try {
    const [tasks, projects] = await Promise.all([
      fetchTasksFromArk(),
      fetchProjectsFromArk(),
    ]);

    store.setTodos(tasks);
    store.setProjects(projects);
    store.setAreas([]);
    store.setTags([]);
    store.setHeadings([]);
    connectionState.value = "online";
  } catch (error) {
    authRequired.value = true;
    authError.value =
      error instanceof Error ? error.message : "Ошибка подключения";
    connectionState.value = "offline";
  } finally {
    store.setHydrated(true);
  }
}

function handleReconnect() {
  if (!isElectron) {
    void bootstrapWeb();
  }
}

async function handleAuthSubmit(connectionCode: string) {
  const parsed = parseConnectionString(connectionCode);

  if (!parsed) {
    authError.value = "Неверный формат. Ожидается: ark://host:port?key=...";
    return;
  }

  authBusy.value = true;
  authError.value = null;

  try {
    setArkUrl(parsed.server_url);
    setArkApiKey(parsed.api_key);
    await bootstrapWeb();
  } finally {
    authBusy.value = false;
  }
}

// ---------------------------------------------------------------------------
// Ark Space handlers (Electron P2P mode)
// ---------------------------------------------------------------------------

async function activateSpace(code: string, seedAddresses: string[] = []) {
  // Clean up existing listeners before setting up new ones (prevents leak on space switch)
  cleanupLanSyncListener?.();
  cleanupLanSyncListener = null;

  const spaceId = await deriveSpaceId(code);
  await setActiveSpace(code);
  // Preserve existing space data (name, createdAt) if already saved
  const existing = (await getSpaces()).find((s) => s.code === code);
  await saveSpace({
    code,
    name: existing?.name ?? formatSpaceCode(code),
    createdAt: existing?.createdAt ?? new Date().toISOString(),
  });
  activeSpaceCode.value = code;
  spaceRequired.value = false;

  // Switch sidecar to per-space DB
  if (window.electronAPI?.invoke) {
    await window.electronAPI
      .invoke("db:switchSpace", spaceId)
      .catch(console.warn);
  }

  // Sync is started via lan-sync:start → ArkClient (see startSyncServer below)

  // Clear ALL store state before loading new space data
  store.setTodos([]);
  store.setProjects([]);
  store.setAreas([]);
  store.setTags([]);
  store.setHeadings([]);
  store.setHydrated(false);

  if (isLocalDbAvailable()) {
    try {
      const { todos, projects, areas, tags, headings } =
        await loadAllFromLocalDb();
      store.setTodos(todos);
      store.setProjects(projects);
      store.setAreas(areas);
      store.setTags(tags);
      if (headings)
        store.setHeadings(headings as import("@/types/task").Heading[]);
    } catch (err) {
      console.warn("[App] Space DB load failed:", err);
    }
  }

  store.setHydrated(true);
  connectionState.value = "syncing";

  // Re-setup IPC bridges after cleanup
  setupLanSyncBridge();

  // Start sync server
  startSyncServer(spaceId, seedAddresses).catch(console.warn);
}

function handleSpaceJoined(code: string, addresses: string[] = []) {
  activateSpace(code, addresses).catch(console.error);
}

function handleSpaceDeleted(code: string) {
  // If the deleted space was the active one, clear it
  if (activeSpaceCode.value === code) {
    handleLeaveSpace();
  }
}

async function handleLeaveSpace() {
  if (window.electronAPI?.invoke) {
    window.electronAPI.invoke("lan-sync:leaveSpace").catch(() => {});
  }
  await setActiveSpace(null);
  activeSpaceCode.value = null;
  connectedPeerCount.value = 0;
  connectedPeerNames.value = [];
  store.setTodos([]);
  store.setProjects([]);
  store.setAreas([]);
  store.setTags([]);
  store.setHeadings([]);
  store.setHydrated(false);
  connectionState.value = "offline";
  spaceRequired.value = true;
}

// ---------------------------------------------------------------------------
// LAN Sync bridge (Electron only)
// ---------------------------------------------------------------------------

function handleLanSyncEntity(entity: SyncEntity) {
  if (!entity || !entity.id) return;

  if (entity.deleted) {
    if (entity.type === "todo") store.removeTodoLocal(entity.id);
    else if (entity.type === "project") store.removeProjectLocal(entity.id);
    return;
  }

  const data = entity.data as Record<string, unknown>;
  switch (entity.type) {
    case "todo":
      store.upsertTodo(data as unknown as import("@/types/task").TodoItem);
      break;
    case "project":
      store.upsertProject(data as unknown as import("@/types/task").Project);
      break;
    // areas, tags, headings -- will add store support later
  }
}

async function startSyncServer(spaceId?: string, seedAddresses: string[] = []) {
  if (!window.electronAPI?.invoke) return;

  const deviceId = localStorage.getItem("delphi.sync_device_id") ?? "unknown";
  // Main process computes the real host name (os.hostname()) and ignores
  // whatever we pass here — send an empty string so we don't pollute logs
  // with the old "Delphi Electron" placeholder.
  const deviceName = "";

  try {
    const started = await window.electronAPI.invoke(
      "lan-sync:start",
      spaceId,
      deviceId,
      deviceName,
      seedAddresses,
    );
    if (!started) {
      connectionState.value = "offline";
      console.warn("[App] Sync server failed to start");
      return;
    }
    console.log("[App] Sync server started");
    refreshPeerStatus();
  } catch (err) {
    connectionState.value = "offline";
    console.warn("[App] Failed to start sync server:", err);
  }
}

function setupLanSyncBridge() {
  if (!window.electronAPI?.on) return;

  // Listen for incoming changes from sync peers
  const unsub1 = window.electronAPI.on(
    "lan-sync:change",
    (...args: unknown[]) => {
      const entity = args[0] as SyncEntity;
      if (entity) handleLanSyncEntity(entity);
    },
  );

  const unsub2 = window.electronAPI.on(
    "lan-sync:peerConnected",
    (...args: unknown[]) => {
      const deviceId = args[0] as string;
      console.log(`[App] Sync peer connected: ${deviceId}`);
      connectedPeerCount.value++;
      connectionState.value = "online";
      // Refresh peer list
      refreshPeerStatus();
    },
  );

  const unsub3 = window.electronAPI.on(
    "lan-sync:peerDisconnected",
    (...args: unknown[]) => {
      const deviceId = args[0] as string;
      const remaining = (args[1] as number) ?? 0;
      console.log(
        `[App] Sync peer disconnected: ${deviceId}, remaining: ${remaining}`,
      );
      connectedPeerCount.value = remaining;
      if (remaining === 0) {
        connectionState.value = "offline";
      }
      refreshPeerStatus();
    },
  );

  cleanupLanSyncListener = () => {
    unsub1?.();
    unsub2?.();
    unsub3?.();
  };
}

/** Refresh peer status from main process. */
function refreshPeerStatus() {
  if (!window.electronAPI?.invoke) return;
  window.electronAPI
    .invoke("lan-sync:getStatus")
    .then(
      (status: { active: boolean; peers: number; peerNames?: string[] }) => {
        connectedPeerCount.value = status.peers;
        connectedPeerNames.value = status.peerNames ?? [];
        connectionState.value = status.peers > 0 ? "online" : "offline";
      },
    )
    .catch(() => {
      connectionState.value = "offline";
    });
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

onMounted(async () => {
  window.addEventListener("keydown", handleGlobalKeydown);
  if (isElectron) {
    // Electron: P2P space mode takes priority
    const code = await getActiveSpace();
    if (code) {
      // Resume existing space
      activeSpaceCode.value = code;
      activateSpace(code).catch(console.error);
    } else {
      // No space -- show setup screen
      spaceRequired.value = true;
      store.setHydrated(true);
      connectionState.value = "offline";
    }
  } else {
    void bootstrapWeb();
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleGlobalKeydown);
  cleanupLanSyncListener?.();
});
</script>

<template>
  <div
    class="flex h-screen w-screen overflow-hidden bg-(--background) text-(--foreground)"
  >
    <!-- Legacy fixed popover replaced by shared titlebar status dot.
    <PopoverRoot>
      <PopoverTrigger as-child>
        <button
          :class="connectionDotClass"
          class="fixed top-4 right-4 z-40 h-2.5 w-2.5 cursor-pointer rounded-full"
        />
      </PopoverTrigger>
      <PopoverPortal>
        <PopoverContent
          side="bottom"
          :side-offset="8"
          align="end"
          class="z-50 w-64 rounded-lg border border-(--border) bg-(--popover) p-3 text-xs text-(--popover-foreground) shadow-md"
        >
          <div class="flex items-center gap-2.5">
            <div
              :class="connectionIconBg"
              class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full"
            >
              <component
                :is="connectionIcon"
                :class="[
                  connectionIconColor,
                  connectionState === 'syncing' && 'animate-spin',
                ]"
                :size="16"
              />
            </div>
            <div class="min-w-0">
              <p class="font-bold">P2P соединение</p>
              <p class="text-(--muted-foreground)">{{ connectionSubtext }}</p>
            </div>
          </div>
          <p
            v-if="activeSpaceCode && connectionState === 'offline'"
            class="mt-2 text-center text-xs text-(--muted-foreground)"
          >
            Ожидание пиров в сети...
          </p>
          <div
            v-if="activeSpaceCode"
            class="mt-3 border-t border-(--border) pt-3"
          >
            <p class="mb-1 text-xs text-(--muted-foreground)">Пространство</p>
            <p class="break-all font-mono text-sm font-bold tracking-wide">
              {{ formatSpaceCode(activeSpaceCode) }}
            </p>
            <button
              class="mt-2 w-full rounded-md border border-(--border) px-2 py-1.5 text-xs font-medium transition-colors hover:bg-(--muted)"
              @click="openQrOverlay"
            >
              Показать QR-код
            </button>
            <div v-if="connectedPeerNames.length > 0" class="mt-2">
              <p class="mb-1 text-xs text-(--muted-foreground)">
                Подключённые пиры
              </p>
              <ul class="text-xs">
                <li v-for="name in connectedPeerNames" :key="name">
                  {{ name }}
                </li>
              </ul>
            </div>
            <button
              class="mt-2 w-full rounded-md border border-rose-500/30 px-2 py-1.5 text-xs font-medium text-rose-400 transition-opacity hover:bg-rose-500/10"
              @click="handleLeaveSpace"
            >
              Покинуть пространство
            </button>
          </div>
          <button
            v-else-if="!isElectron && connectionState === 'offline'"
            class="mt-3 w-full rounded-md bg-(--foreground) px-2 py-1.5 text-xs font-medium text-(--background) transition-opacity hover:opacity-80"
            @click="handleReconnect"
          >
            Подключиться
          </button>
          <PopoverArrow class="fill-(--popover)" />
        </PopoverContent>
      </PopoverPortal>
    </PopoverRoot>
    -->

    <DesktopChrome :platform="chromePlatform" class="flex min-h-0 min-w-0 flex-1">
      <template #titlebar-leading>
        <button
          type="button"
          class="inline-flex h-8 min-w-8 items-center justify-center rounded-[10px] px-2 text-(--muted-foreground) transition-colors hover:bg-white/8 hover:text-(--foreground)"
          :title="sidebarHidden ? 'Показать боковую панель' : 'Скрыть боковую панель'"
          @click="toggleSidebar"
        >
          <PanelLeft :size="14" />
        </button>

        <TitlebarHistoryControls
          :back-disabled="!canGoBack"
          :forward-disabled="!canGoForward"
          back-title="Назад"
          forward-title="Вперёд"
          @back="navigateBack"
          @forward="navigateForward"
        />
      </template>

      <template #titlebar-trailing>
        <StatusDot
          :tone="connectionStatusTone"
          :label="connectionStatusLabel"
        >
          <div class="w-64 text-xs text-(--popover-foreground)">
            <div class="flex items-center gap-2.5">
              <div
                :class="connectionIconBg"
                class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full"
              >
                <component
                  :is="connectionIcon"
                  :class="[
                    connectionIconColor,
                    connectionState === 'syncing' && 'animate-spin',
                  ]"
                  :size="16"
                />
              </div>
              <div class="min-w-0">
                <p class="font-bold">P2P СЃРѕРµРґРёРЅРµРЅРёРµ</p>
                <p class="text-(--muted-foreground)">{{ connectionSubtext }}</p>
              </div>
            </div>
            <p
              v-if="activeSpaceCode && connectionState === 'offline'"
              class="mt-2 text-center text-xs text-(--muted-foreground)"
            >
              РћР¶РёРґР°РЅРёРµ РїРёСЂРѕРІ РІ СЃРµС‚Рё...
            </p>
            <div
              v-if="activeSpaceCode"
              class="mt-3 border-t border-(--border) pt-3"
            >
              <p class="mb-1 text-xs text-(--muted-foreground)">РџСЂРѕСЃС‚СЂР°РЅСЃС‚РІРѕ</p>
              <p class="break-all font-mono text-sm font-bold tracking-wide">
                {{ formatSpaceCode(activeSpaceCode) }}
              </p>
              <button
                class="mt-2 w-full rounded-md border border-(--border) px-2 py-1.5 text-xs font-medium transition-colors hover:bg-(--muted)"
                @click="openQrOverlay"
              >
                РџРѕРєР°Р·Р°С‚СЊ QR-РєРѕРґ
              </button>
              <div v-if="connectedPeerNames.length > 0" class="mt-2">
                <p class="mb-1 text-xs text-(--muted-foreground)">
                  РџРѕРґРєР»СЋС‡С‘РЅРЅС‹Рµ РїРёСЂС‹
                </p>
                <ul class="text-xs">
                  <li v-for="name in connectedPeerNames" :key="name">
                    {{ name }}
                  </li>
                </ul>
              </div>
              <button
                class="mt-2 w-full rounded-md border border-rose-500/30 px-2 py-1.5 text-xs font-medium text-rose-400 transition-opacity hover:bg-rose-500/10"
                @click="handleLeaveSpace"
              >
                РџРѕРєРёРЅСѓС‚СЊ РїСЂРѕСЃС‚СЂР°РЅСЃС‚РІРѕ
              </button>
            </div>
            <button
              v-else-if="!isElectron && connectionState === 'offline'"
              class="mt-3 w-full rounded-md bg-(--foreground) px-2 py-1.5 text-xs font-medium text-(--background) transition-opacity hover:opacity-80"
              @click="handleReconnect"
            >
              РџРѕРґРєР»СЋС‡РёС‚СЊСЃСЏ
            </button>
          </div>
        </StatusDot>
      </template>

      <template #sidebar>
        <SideBar
          :hidden="sidebarHidden"
          :show-toggle="false"
          :reserve-top-inset="false"
        />
      </template>

      <DesktopContentSurface
        class="flex min-h-0 min-w-0 flex-1"
        padding-top="0"
        padding-inline="0"
        padding-bottom="0"
        :show-left-border="!sidebarHidden"
        :radius-top-left="sidebarHidden ? '0px' : '16px'"
      >
        <main class="flex min-h-0 min-w-0 flex-1 flex-col">
          <RouterView />
        </main>
      </DesktopContentSurface>
    </DesktopChrome>

    <QuickEntry />
    <QuickSearch v-model:open="quickSearchOpen" />
    <CustomCaret />

    <AuthOverlay
      v-if="!isElectron && authRequired"
      :busy="authBusy"
      :error-message="authError"
      @submit="handleAuthSubmit"
    />

    <SpaceSetup
      v-if="spaceRequired"
      @space-joined="handleSpaceJoined"
      @space-deleted="handleSpaceDeleted"
    />

    <!-- Fullscreen QR overlay -->
    <div
      v-if="showQrOverlay"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-8"
      @click.self="showQrOverlay = false"
    >
      <div
        class="flex flex-col items-center gap-4 rounded-2xl bg-(--background) p-8 shadow-2xl"
      >
        <p class="text-xs text-(--muted-foreground)">Пространство</p>
        <p
          v-if="activeSpaceCode"
          class="font-mono text-2xl font-bold tracking-widest"
        >
          {{ formatSpaceCode(activeSpaceCode) }}
        </p>
        <img
          v-if="fullQrDataUrl"
          :src="fullQrDataUrl"
          alt="QR"
          class="rounded-lg"
          width="320"
          height="320"
        />
        <p class="max-w-xs text-center text-xs text-(--muted-foreground)">
          Отсканируйте QR или вставьте ссылку на другом устройстве
        </p>
        <div class="flex w-full gap-2">
          <button
            v-if="qrOverlayPayload"
            class="flex-1 rounded-md bg-(--foreground) px-4 py-2 text-sm font-medium text-(--background) transition-opacity hover:opacity-80"
            @click="copyQrLink"
          >
            {{ qrLinkCopied ? "Скопировано!" : "Скопировать ссылку" }}
          </button>
          <button
            class="rounded-md border border-(--border) px-4 py-2 text-sm transition-colors hover:bg-(--muted)"
            @click="showQrOverlay = false"
          >
            Закрыть
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
