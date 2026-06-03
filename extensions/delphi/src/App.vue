<script setup lang="ts">
/* eslint-disable no-console */
import { computed, onMounted, onUnmounted, shallowRef, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import { PanelLeft } from "@lucide/vue";
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
import type { Area, Heading, Project, Tag, TodoItem } from "@/types/task";
import { useTodoStore } from "@/store/todos";
import {
  arkGetTask,
  DELPHI_TASK_OBJ_TYPE_ID,
  subscribeArkObjectChanges,
} from "@/lib/electron-api-shim";
import SideBar from "@/components/SideBar.vue";
import QuickEntry from "@/components/QuickEntry.vue";
import QuickSearch from "@/components/QuickSearch.vue";
import AuthOverlay from "@/components/AuthOverlay.vue";
import SpaceSetup from "@/components/SpaceSetup.vue";
import { useQuickEntry } from "@/composables/useQuickEntry";
import {
  DesktopChrome,
  DesktopContentSurface,
  StatusDot,
  type StatusDotTone,
  TitlebarHistoryControls,
  type TitlebarPlatform,
} from "@kosmos/visuals";
import { setSidebarHidden, useSidebarState } from "@/composables/useSidebarState";
import {
  activeSpaceCode,
  arkStatus,
  connectedPeerCount,
  connectedPeerNames,
  connectionState,
  LEAVE_SPACE_EVENT,
} from "@/composables/useSyncState";

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

const store = useTodoStore();
const route = useRoute();
const router = useRouter();
const { show: showQuickEntry } = useQuickEntry();

const quickSearchOpen = shallowRef(false);
const { sidebarHidden } = useSidebarState();
const backStack = shallowRef<string[]>([]);
const forwardStack = shallowRef<string[]>([]);
const historyReady = shallowRef(false);
const suppressHistoryRecording = shallowRef(false);

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

const authRequired = shallowRef(false);
const authBusy = shallowRef(false);
const authError = shallowRef<string | null>(null);

const isElectron = typeof window !== "undefined" && !!window.electronAPI;
const spaceRequired = shallowRef(false);

const arkStatusMessage = computed(() => {
  switch (arkStatus.value) {
    case "connected":
      return "ARK подключен";
    case "connecting":
      return "Подключение к ARK…";
    case "error":
    default:
      return "ARK недоступен";
  }
});

const arkStatusTone = computed<StatusDotTone>(() => {
  switch (arkStatus.value) {
    case "connected":
      return "success";
    case "connecting":
      return "warning";
    case "error":
    default:
      return "danger";
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

function openSettings() {
  if (route.path === "/settings") return;
  void router.push("/settings");
}

const browserHistoryState = computed(() => {
  void route.fullPath;

  if (typeof window === "undefined") return undefined;

  return (window.history.state as RouterHistoryStateLike | null) ?? undefined;
});

const canExitSettingsViaBack = computed(() => route.path === "/settings");

const canGoBack = computed(() => {
  if (isElectron) return backStack.value.length > 0 || canExitSettingsViaBack.value;
  return Boolean(browserHistoryState.value?.back) || canExitSettingsViaBack.value;
});

const canGoForward = computed(() => {
  if (isElectron) return forwardStack.value.length > 0;
  return Boolean(browserHistoryState.value?.forward);
});

async function navigateBack() {
  if (isElectron) {
    const targetPath = backStack.value.at(-1);
    if (!targetPath) {
      if (!canExitSettingsViaBack.value) return;
      await router.push("/");
      return;
    }

    suppressHistoryRecording.value = true;
    backStack.value = backStack.value.slice(0, -1);
    forwardStack.value = [...forwardStack.value, route.fullPath];

    try {
      await router.push(targetPath);
    } finally {
      suppressHistoryRecording.value = false;
    }

    return;
  }

  if (!browserHistoryState.value?.back) {
    if (!canExitSettingsViaBack.value) return;
    await router.push("/");
    return;
  }
  router.back();
}

async function navigateForward() {
  if (isElectron) {
    const targetPath = forwardStack.value.at(-1);
    if (!targetPath) return;

    suppressHistoryRecording.value = true;
    forwardStack.value = forwardStack.value.slice(0, -1);
    backStack.value = [...backStack.value, route.fullPath];

    try {
      await router.push(targetPath);
    } finally {
      suppressHistoryRecording.value = false;
    }

    return;
  }

  if (!browserHistoryState.value?.forward) return;
  router.forward();
}

watch(
  () => route.fullPath,
  (nextPath, previousPath) => {
    if (!isElectron) return;

    if (!historyReady.value) {
      historyReady.value = true;
      return;
    }

    if (suppressHistoryRecording.value || !previousPath || nextPath === previousPath) {
      return;
    }

    backStack.value = [...backStack.value, previousPath];
    forwardStack.value = [];
  },
);

// ---------------------------------------------------------------------------
// Cleanup registry
// ---------------------------------------------------------------------------

let cleanupLanSyncListener: (() => void) | null = null;
let cleanupCommandListeners: (() => void) | null = null;

function setupCommandBusBridge() {
  if (!window.electronAPI?.on) return;

  const unsubCreate = window.electronAPI.on("delphi:cmd:task:create", () => {
    showQuickEntry();
  });

  const unsubToday = window.electronAPI.on("delphi:cmd:task:today", () => {
    if (route.path !== "/today") {
      void router.push("/today");
    }
  });

  cleanupCommandListeners = () => {
    unsubCreate?.();
    unsubToday?.();
  };
}

function mergeTodosById(localTodos: TodoItem[], arkTodos: TodoItem[]): TodoItem[] {
  const byId = new Map<string, TodoItem>();

  for (const todo of localTodos) {
    byId.set(todo.id, todo);
  }

  for (const todo of arkTodos) {
    byId.set(todo.id, todo);
  }

  return [...byId.values()];
}

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
  arkStatus.value = "connecting";
  authRequired.value = false;
  authError.value = null;
  store.setHydrated(false);

  try {
    const [tasks, projects] = await Promise.all([fetchTasksFromArk(), fetchProjectsFromArk()]);

    store.setTodos(tasks);
    store.setProjects(projects);
    store.setAreas([]);
    store.setTags([]);
    store.setHeadings([]);
    connectionState.value = "online";
    arkStatus.value = "connected";
  } catch (error) {
    authRequired.value = true;
    authError.value = error instanceof Error ? error.message : "Ошибка подключения";
    connectionState.value = "offline";
    arkStatus.value = "error";
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
  arkStatus.value = "connecting";
  if (window.electronAPI?.invoke) {
    try {
      await window.electronAPI.invoke("db:switchSpace", spaceId);
      arkStatus.value = "connected";
    } catch (err) {
      console.warn("[App] db:switchSpace failed:", err);
      arkStatus.value = "error";
    }
  }

  // Sync is started via lan-sync:start → ArkClient (see startSyncServer below)

  // Clear ALL store state before loading new space data
  store.setTodos([]);
  store.setProjects([]);
  store.setAreas([]);
  store.setTags([]);
  store.setHeadings([]);
  store.setHydrated(false);

  // Если есть local DB (legacy standalone Delphi) — грузим оттуда + merge
  // с ARK. В extension context'е local DB unavailable, поэтому ARK load
  // должен происходить ВСЕГДА, независимо от isLocalDbAvailable —
  // раньше всё было внутри if (isLocalDbAvailable()) → в extension store
  // оставался пустым даже когда в ARK есть задачи.
  let mergedTodos: TodoItem[] = [];
  let mergedProjects: Project[] = [];
  let mergedAreas: Area[] = [];
  let mergedTags: Tag[] = [];
  let mergedHeadings: Heading[] = [];

  if (isLocalDbAvailable()) {
    try {
      const { todos, projects, areas, tags, headings } = await loadAllFromLocalDb();
      mergedTodos = todos;
      mergedProjects = projects;
      mergedAreas = areas;
      mergedTags = tags;
      mergedHeadings = (headings ?? []) as Heading[];
    } catch (err) {
      console.warn("[App] Space DB load failed:", err);
    }
  }

  // ARK task load — всегда, не зависит от local DB availability.
  if (window.electronAPI?.invoke) {
    try {
      const arkTodos = (await window.electronAPI.invoke("ark:listDelphiTasks")) as TodoItem[];
      if (Array.isArray(arkTodos) && arkTodos.length > 0) {
        mergedTodos = mergeTodosById(mergedTodos, arkTodos);
      }
    } catch (err) {
      console.warn("[App] Ark task load failed:", err);
    }
  }

  store.setTodos(mergedTodos);
  store.setProjects(mergedProjects);
  store.setAreas(mergedAreas);
  store.setTags(mergedTags);
  store.setHeadings(mergedHeadings);

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
  const unsub1 = window.electronAPI.on("lan-sync:change", (...args: unknown[]) => {
    const entity = args[0] as SyncEntity;
    if (entity) handleLanSyncEntity(entity);
  });

  const unsub2 = window.electronAPI.on("lan-sync:peerConnected", (...args: unknown[]) => {
    const deviceId = args[0] as string;
    console.log(`[App] Sync peer connected: ${deviceId}`);
    connectedPeerCount.value++;
    connectionState.value = "online";
    // Refresh peer list
    refreshPeerStatus();
  });

  const unsub3 = window.electronAPI.on("lan-sync:peerDisconnected", (...args: unknown[]) => {
    const deviceId = args[0] as string;
    const remaining = (args[1] as number) ?? 0;
    console.log(`[App] Sync peer disconnected: ${deviceId}, remaining: ${remaining}`);
    connectedPeerCount.value = remaining;
    if (remaining === 0) {
      connectionState.value = "offline";
    }
    refreshPeerStatus();
  });

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
    .then((status: { active: boolean; peers: number; peerNames?: string[] }) => {
      connectedPeerCount.value = status.peers;
      connectedPeerNames.value = status.peerNames ?? [];
      connectionState.value = status.peers > 0 ? "online" : "offline";
    })
    .catch(() => {
      connectionState.value = "offline";
    });
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// ARK live updates (cross-app: Eden /задача → Delphi UI без reload)
// ---------------------------------------------------------------------------
//
// Подписка на `object_upserted` / `object_deleted` для task_obj. Pattern B —
// renderer держит cache, ARK events его инвалидируют. См.
// `lib/electron-api-shim.ts → subscribeArkObjectChanges`.

let cleanupArkObjectListener: (() => void) | null = null;

function setupArkObjectListener() {
  cleanupArkObjectListener?.();
  cleanupArkObjectListener = subscribeArkObjectChanges(async (change) => {
    if (change.event === "object_upserted") {
      if (change.typeId !== DELPHI_TASK_OBJ_TYPE_ID) return;
      const todo = await arkGetTask(change.id);
      if (!todo) {
        // Объект помечен deleted или type mismatch — снимаем из кэша если был.
        if (store.todos.some((t) => t.id === change.id)) {
          store.removeTodoLocal(change.id);
        }
        return;
      }
      store.upsertTodo(todo);
      return;
    }
    // object_deleted: type_id отсутствует в payload — проверяем по cache.
    if (store.todos.some((t) => t.id === change.id)) {
      store.removeTodoLocal(change.id);
    }
  });
}

onMounted(async () => {
  window.addEventListener("keydown", handleGlobalKeydown);
  window.addEventListener(LEAVE_SPACE_EVENT, leaveSpaceListener);
  setupArkObjectListener();
  if (isElectron) {
    setupCommandBusBridge();
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
  window.removeEventListener(LEAVE_SPACE_EVENT, leaveSpaceListener);
  cleanupLanSyncListener?.();
  cleanupCommandListeners?.();
  cleanupArkObjectListener?.();
  cleanupArkObjectListener = null;
});

function leaveSpaceListener() {
  void handleLeaveSpace();
}
</script>

<template>
  <div class="flex h-screen w-screen overflow-hidden bg-(--background) text-(--foreground)">
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
        <button
          type="button"
          :class="[
            'titlebar-settings-button',
            route.path === '/settings' ? 'titlebar-settings-button--active' : '',
          ]"
          title="Настройки"
          @click="openSettings"
        >
          <span>Настройки</span>
        </button>
      </template>

      <template #titlebar-trailing>
        <StatusDot :tone="arkStatusTone" :label="arkStatusMessage" />
      </template>

      <template #sidebar>
        <SideBar :hidden="sidebarHidden" :show-toggle="false" :reserve-top-inset="false" />
      </template>

      <DesktopContentSurface
        class="flex min-h-0 min-w-0 flex-1"
        padding-top="0"
        padding-inline="0"
        padding-bottom="0"
        :show-left-border="!sidebarHidden"
        :radius-top-left="sidebarHidden ? '0px' : '16px'"
      >
        <main class="relative flex min-h-0 min-w-0 flex-1 flex-col">
          <RouterView />
          <QuickEntry />
        </main>
      </DesktopContentSurface>
    </DesktopChrome>

    <QuickSearch v-model:open="quickSearchOpen" />

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
  </div>
</template>

<style scoped>
.titlebar-settings-button {
  display: inline-flex;
  align-items: center;
  justify-content: flex-start;
  min-height: 0;
  padding: 4px;
  border-radius: calc(var(--radius) * 1.4);
  corner-shape: var(--corner-shape);
  color: var(--muted-foreground);
  font-size: 0.6875rem;
  line-height: 1rem;
  font-weight: 500;
  opacity: 0.9;
  text-align: left;
  transition:
    background-color 120ms var(--easing-emphasized),
    color 120ms var(--easing-emphasized),
    opacity 120ms var(--easing-emphasized);
}

.titlebar-settings-button:hover {
  background: color-mix(in srgb, var(--sidebar-foreground) 8%, transparent);
}

.titlebar-settings-button--active {
  background: color-mix(in srgb, var(--sidebar-foreground) 10%, transparent);
  color: var(--foreground);
  opacity: 1;
}
</style>
