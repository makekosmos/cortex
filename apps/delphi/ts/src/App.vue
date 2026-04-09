<script setup lang="ts">
/* eslint-disable no-console */
import { computed, onMounted, onUnmounted, shallowRef, watch } from "vue";
import { RouterView } from "vue-router";
import {
  PopoverArrow,
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger,
} from "reka-ui";
import { Loader, Wifi, WifiOff } from "lucide-vue-next";
import QRCode from "qrcode";
import {
  type ArkChange,
  arkChangeEventType,
  arkChangeToProject,
  arkChangeToTodoItem,
  arkSync,
  fetchProjectsFromArk,
  fetchTasksFromArk,
  getArkApiKey,
  getArkUrl,
  setArkApiKey,
  setArkUrl,
  todoItemToArkChange,
} from "@/services/sync/ark-client";
import {
  isLocalDbAvailable,
  loadAllFromLocalDb,
  localDbBatchUpsertTodos,
  localDbUpsertProject,
} from "@/services/storage/local-db";
import { parseConnectionString } from "@/services/sync/pairing";
import {
  broadcastToPeers,
  setupMeshFromArkKey,
  setupMeshFromSpaceCode,
  stopMesh,
} from "@/services/sync/peer-bridge";
import {
  deriveSpaceId,
  formatSpaceCode,
  generateQrPayload,
  getActiveSpace,
  getSpaces,
  saveSpace,
  setActiveSpace,
} from "@/services/space/space-manager";
import type { SyncEntity, SyncEntityType } from "@/services/sync/lan-protocol";
import { useTodoStore } from "@/store/todos";
import SideBar from "@/components/SideBar.vue";
import QuickEntry from "@/components/QuickEntry.vue";
import QuickSearch from "@/components/QuickSearch.vue";
import AuthOverlay from "@/components/AuthOverlay.vue";
import SpaceSetup from "@/components/SpaceSetup.vue";
import { CustomCaret } from "@kepler/visuals/components";

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

const store = useTodoStore();

const quickSearchOpen = shallowRef(false);

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

const SYNC_TIMEOUT = 10_000;

const connectionState = shallowRef<ConnectionState>("syncing");
const authRequired = shallowRef(false);
const authBusy = shallowRef(false);
const authError = shallowRef<string | null>(null);
let syncTimer: ReturnType<typeof setTimeout> | null = null;

// Ark Space state (Electron P2P mode)
const isElectron = typeof window !== "undefined" && !!window.electronAPI;
const spaceRequired = shallowRef(false);
const activeSpaceCode = shallowRef<string | null>(null);
const peerServerAddress = shallowRef<string | null>(null);
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
    setTimeout(() => (qrLinkCopied.value = false), 2000);
  } catch {
    /* clipboard blocked */
  }
}

const connectionDotClass = computed(() => {
  switch (connectionState.value) {
    case "online":
      return "bg-emerald-500";
    case "syncing":
      return "bg-amber-500 animate-pulse";
    case "offline":
      return "bg-rose-500";
  }
});

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

// ---------------------------------------------------------------------------
// Shared change handler (used by both Ark WS and P2P peer bridge)
// ---------------------------------------------------------------------------

function handleArkChange(change: ArkChange) {
  const eventType = arkChangeEventType(change);

  if (eventType === "project") {
    if (change.change_type === "delete") {
      store.removeProject(change.event_id);
      return;
    }
    const project = arkChangeToProject(change);
    if (project) store.upsertProject(project);
    return;
  }

  // Default: handle as task
  if (change.change_type === "delete") {
    store.removeTodo(change.event_id);
    return;
  }
  const todo = arkChangeToTodoItem(change);
  if (todo) store.upsertTodo(todo);
}

// ---------------------------------------------------------------------------
// Cleanup registry
// ---------------------------------------------------------------------------

let unsubStatus: (() => void) | null = null;
let unsubChange: (() => void) | null = null;
let cleanupPeerListener: (() => void) | null = null;
let unsubFullSync: (() => void) | null = null;
let cleanupLanSyncListener: (() => void) | null = null;

// ---------------------------------------------------------------------------
// Bootstrap: connect to Ark, setup listeners
// ---------------------------------------------------------------------------

function bootstrap() {
  // Unsubscribe previous listeners if re-bootstrapping
  unsubStatus?.();
  unsubChange?.();

  // Subscribe to sync status changes
  unsubStatus = arkSync.onStatus((connected) => {
    if (syncTimer) {
      clearTimeout(syncTimer);
      syncTimer = null;
    }
    connectionState.value = connected ? "online" : "offline";
  });

  // Subscribe to sync change events
  unsubChange = arkSync.onChange(handleArkChange);

  unsubFullSync = arkSync.onFullSync((serverTaskIds, outboxTaskIds) => {
    // Delete zombie tasks: exist locally but server doesn't know about them
    // and they're not pending to be sent. These are duplicates from old buggy syncs.
    const zombies = store.todos.value.filter(
      (t) =>
        !serverTaskIds.has(t.id.toLowerCase()) &&
        !outboxTaskIds.has(t.id.toLowerCase()),
    );
    if (zombies.length > 0) {
      console.log(
        `[App] Removing ${zombies.length} zombie tasks after full sync`,
      );
      zombies.forEach((t) => store.removeTodoLocal(t.id));
    }
  });

  // Register a resolver so flushOutbox can re-read current task state and
  // avoid pushing stale outbox payloads after receiving sync_changes.
  arkSync.setTodoResolver((id) => store.todos.value.find((t) => t.id === id));
  arkSync.setAllTodosResolver(() => store.todos.value);

  const url = getArkUrl();
  const key = getArkApiKey();

  if (!url || !key) {
    authRequired.value = true;
    connectionState.value = "offline";
    store.setHydrated(true);
    return;
  }

  // Load tasks: local SQLite first (fast, offline), then HTTP fallback if DB is empty
  if (isLocalDbAvailable()) {
    loadAllFromLocalDb()
      .then(({ todos, projects, areas, tags }) => {
        if (todos.length > 0) store.setTodos(todos);
        if (projects.length > 0) store.setProjects(projects);
        if (areas.length > 0) store.setAreas(areas);
        if (tags.length > 0) store.setTags(tags);
        console.log(
          `[App] Loaded from local DB: ${todos.length} todos, ${projects.length} projects`,
        );
        // First run: local DB is empty, bootstrap from Ark HTTP to populate it
        if (todos.length === 0) {
          Promise.all([fetchTasksFromArk(), fetchProjectsFromArk()])
            .then(([tasks, projs]) => {
              if (tasks.length > 0) store.setTodos(tasks);
              if (projs.length > 0) store.setProjects(projs);
              // Persist to local DB so next launch is instant
              void localDbBatchUpsertTodos(tasks);
              projs.forEach((p) => void localDbUpsertProject(p));
            })
            .catch((err) => console.warn("[App] HTTP bootstrap failed:", err));
        }
      })
      .catch((err) => console.warn("[App] Local DB load failed:", err));
  } else {
    Promise.all([fetchTasksFromArk(), fetchProjectsFromArk()])
      .then(([tasks, projects]) => {
        if (tasks.length > 0) store.setTodos(tasks);
        if (projects.length > 0) store.setProjects(projects);
      })
      .catch((err) => console.warn("[App] Failed to bootstrap from Ark:", err));
  }

  // Connect WebSocket for realtime sync
  if (!arkSync.isConnected) {
    arkSync.connect(url, key);
  }
  authRequired.value = false;
  authError.value = null;
  connectionState.value = "syncing";
  store.setHydrated(true);

  if (syncTimer) clearTimeout(syncTimer);
  syncTimer = setTimeout(() => {
    if (connectionState.value === "syncing") {
      connectionState.value = "offline";
    }
    syncTimer = null;
  }, SYNC_TIMEOUT);
}

// ---------------------------------------------------------------------------
// Manual reconnect
// ---------------------------------------------------------------------------

function handleReconnect() {
  const url = getArkUrl();
  const key = getArkApiKey();
  if (!url || !key) {
    authRequired.value = true;
    return;
  }
  connectionState.value = "syncing";
  arkSync.disconnect();
  arkSync.connect(url, key);

  if (syncTimer) clearTimeout(syncTimer);
  syncTimer = setTimeout(() => {
    if (connectionState.value === "syncing") {
      connectionState.value = "offline";
    }
    syncTimer = null;
  }, SYNC_TIMEOUT);
}

// ---------------------------------------------------------------------------
// Ark Space handlers (Electron P2P mode)
// ---------------------------------------------------------------------------

async function activateSpace(code: string, seedAddresses: string[] = []) {
  // Clean up existing listeners before setting up new ones (prevents leak on space switch)
  cleanupPeerListener?.();
  cleanupPeerListener = null;
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

  // Start P2P mesh with space code as shared secret
  setupMeshFromSpaceCode(code);

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
  connectionState.value = "offline"; // P2P -- Ark WS not needed

  // Get the server address for manual connect display
  if (window.electronAPI?.invoke) {
    window.electronAPI
      .invoke("peer:getServerAddress")
      .then((addr: string | null) => {
        peerServerAddress.value = addr;
      })
      .catch(() => {});
  }

  // Re-setup IPC bridges after cleanup
  setupPeerBridge();
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
  stopMesh();
  if (window.electronAPI?.invoke) {
    window.electronAPI.invoke("lan-sync:stop").catch(() => {});
  }
  await setActiveSpace(null);
  activeSpaceCode.value = null;
  connectedPeerCount.value = 0;
  connectedPeerNames.value = [];
  store.setTodos([]);
  store.setProjects([]);
  store.setAreas([]);
  store.setTags([]);
  store.setHydrated(false);
  spaceRequired.value = true;
}

// ---------------------------------------------------------------------------
// P2P peer bridge (Electron only)
// ---------------------------------------------------------------------------

function setupPeerBridge() {
  if (!window.electronAPI?.on) return;

  cleanupPeerListener = window.electronAPI.on(
    "peer:change",
    (...args: unknown[]) => {
      const change = args[0] as ArkChange;
      if (!change || !change.event_id) return;
      handleArkChange(change);
    },
  );
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

/** Broadcast a local change to sync peers. */
function broadcastToLanSync(
  entityType: SyncEntityType,
  id: string,
  data: Record<string, unknown>,
  deleted?: boolean,
) {
  if (!window.electronAPI?.invoke) return;
  const entity: SyncEntity = {
    type: entityType,
    id,
    data,
    hlc: "", // will be set by main process
    deleted,
  };
  window.electronAPI
    .invoke("lan-sync:broadcastChange", entity)
    .catch((err: unknown) => {
      console.warn("[LanSync] Failed to broadcast change:", err);
    });
}

async function startSyncServer(spaceId?: string, seedAddresses: string[] = []) {
  if (!window.electronAPI?.invoke) return;

  const deviceId = localStorage.getItem("delphi.sync_device_id") ?? "unknown";
  // Main process computes the real host name (os.hostname()) and ignores
  // whatever we pass here — send an empty string so we don't pollute logs
  // with the old "Delphi Electron" placeholder.
  const deviceName = "";

  try {
    await window.electronAPI.invoke(
      "lan-sync:start",
      spaceId,
      deviceId,
      deviceName,
      seedAddresses,
    );
    console.log("[App] Sync server started");
  } catch (err) {
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
      },
    )
    .catch(() => {});
}

// ---------------------------------------------------------------------------
// Auth handler
// ---------------------------------------------------------------------------

async function handleAuthSubmit(connectionCode: string) {
  const parsed = parseConnectionString(connectionCode);

  if (!parsed) {
    authError.value = "Неверный формат. Ожидается: ark://host:port?key=...";
    return;
  }

  authBusy.value = true;
  authError.value = null;
  connectionState.value = "syncing";

  try {
    setArkUrl(parsed.server_url);
    setArkApiKey(parsed.api_key);
    arkSync.connect(parsed.server_url, parsed.api_key);
    setupMeshFromArkKey(parsed.api_key);
    authRequired.value = false;
    // Re-bootstrap to fetch initial data with new credentials
    bootstrap();
  } catch (error) {
    authError.value =
      error instanceof Error ? error.message : "Ошибка подключения";
    connectionState.value = "offline";
  } finally {
    authBusy.value = false;
  }
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

    setupPeerBridge();
    setupLanSyncBridge();

    // Old P2P peer bridge: only push local state when LAN sync is NOT active.
    if (window.electronAPI?.on && !activeSpaceCode.value) {
      window.electronAPI.on("peer:peerConnected", (...args: unknown[]) => {
        const deviceId = args[0] as string;
        console.log(
          `[App] P2P peer connected: ${deviceId} -- pushing local state`,
        );
        connectionState.value = "online";
        const currentTodos = store.todos ?? [];
        const currentProjects = store.projects ?? [];
        currentTodos.forEach((todo) => {
          broadcastToPeers(todoItemToArkChange(todo, "update"));
        });
        currentProjects.forEach((project) => {
          broadcastToPeers({
            event_id: project.id,
            change_type: "update",
            data: {
              event_type: "project",
              category: "productivity",
              source: "delphi-web",
              source_id: project.id,
              summary: project.title,
              occurred_at: new Date().toISOString(),
              data: project,
            },
          });
        });
      });

      window.electronAPI.on("peer:peerDisconnected", (...args: unknown[]) => {
        const deviceId = args[0] as string;
        const remaining = (args[1] as number) ?? 0;
        console.log(
          `[App] P2P peer disconnected: ${deviceId}, remaining: ${remaining}`,
        );
        if (remaining === 0) {
          connectionState.value = "offline";
        }
      });
    }
  } else {
    // Web mode: classic Ark WS auth flow
    bootstrap();
    setupPeerBridge();

    const key = getArkApiKey();
    if (key) {
      setupMeshFromArkKey(key);
    }
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleGlobalKeydown);
  unsubStatus?.();
  unsubChange?.();
  unsubFullSync?.();
  cleanupPeerListener?.();
  cleanupLanSyncListener?.();
  if (syncTimer) clearTimeout(syncTimer);
  arkSync.disconnect();
});
</script>

<template>
  <div
    class="flex h-screen w-screen overflow-hidden bg-(--background) text-(--foreground)"
  >
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
          <button
            v-if="connectionState === 'offline' && !activeSpaceCode"
            class="mt-3 w-full rounded-md bg-(--foreground) px-2 py-1.5 text-xs font-medium text-(--background) transition-opacity hover:opacity-80"
            @click="handleReconnect"
          >
            Подключиться
          </button>
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
          <PopoverArrow class="fill-(--popover)" />
        </PopoverContent>
      </PopoverPortal>
    </PopoverRoot>

    <SideBar />

    <main class="flex min-h-0 min-w-0 flex-1 flex-col">
      <RouterView />
    </main>

    <QuickEntry />
    <QuickSearch v-model:open="quickSearchOpen" />
    <CustomCaret />

    <AuthOverlay
      v-if="authRequired"
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
