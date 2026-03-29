<script setup lang="ts">
import { shallowRef, computed, onMounted, onUnmounted } from "vue";
import { RouterView } from "vue-router";
import {
  PopoverRoot,
  PopoverTrigger,
  PopoverPortal,
  PopoverContent,
  PopoverArrow,
} from "reka-ui";
import { Wifi, WifiOff, Loader } from "lucide-vue-next";
import {
  type ArkChange,
  arkChangeToTodoItem,
  arkChangeToProject,
  arkChangeEventType,
  arkSync,
  getArkApiKey,
  getArkUrl,
  setArkApiKey,
  setArkUrl,
  fetchTasksFromArk,
  fetchProjectsFromArk,
} from "@/services/sync/ark-client";
import { parseConnectionString } from "@/services/sync/pairing";
import { setupMeshFromArkKey } from "@/services/sync/peer-bridge";
import { useTodoStore } from "@/store/todos";
import SideBar from "@/components/SideBar.vue";
import QuickEntry from "@/components/QuickEntry.vue";
import QuickOpen from "@/components/QuickOpen.vue";
import AuthOverlay from "@/components/AuthOverlay.vue";

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

const store = useTodoStore();

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

  const url = getArkUrl();
  const key = getArkApiKey();

  if (!url || !key) {
    authRequired.value = true;
    connectionState.value = "offline";
    store.setHydrated(true);
    return;
  }

  // Load tasks and projects from Ark HTTP API
  Promise.all([fetchTasksFromArk(), fetchProjectsFromArk()])
    .then(([tasks, projects]) => {
      if (tasks.length > 0) store.setTodos(tasks);
      if (projects.length > 0) store.setProjects(projects);
    })
    .catch((err) => {
      console.warn("[App] Failed to bootstrap from Ark:", err);
    });

  // Connect WebSocket for realtime sync
  if (!arkSync.isConnected) {
    arkSync.connect(url, key);
  }
  authRequired.value = false;
  authError.value = null;
  connectionState.value = "syncing";
  store.setHydrated(true);

  // Если за SYNC_TIMEOUT соединение не установилось — переключаем в offline
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

onMounted(() => {
  bootstrap();
  setupPeerBridge();

  // Auto-setup mesh credentials from existing Ark key (Electron only)
  const key = getArkApiKey();
  if (key) {
    setupMeshFromArkKey(key);
  }
});

onUnmounted(() => {
  unsubStatus?.();
  unsubChange?.();
  cleanupPeerListener?.();
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
          class="z-50 w-56 rounded-lg border border-(--border) bg-(--popover) p-3 text-xs text-(--popover-foreground) shadow-md"
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
            v-if="connectionState === 'offline'"
            class="mt-3 w-full rounded-md bg-(--foreground) px-2 py-1.5 text-xs font-medium text-(--background) transition-opacity hover:opacity-80"
            @click="handleReconnect"
          >
            Подключиться
          </button>
          <PopoverArrow class="fill-(--popover)" />
        </PopoverContent>
      </PopoverPortal>
    </PopoverRoot>

    <SideBar />

    <main class="flex min-h-0 min-w-0 flex-1 flex-col">
      <RouterView />
    </main>

    <QuickEntry />
    <QuickOpen />

    <AuthOverlay
      v-if="authRequired"
      :busy="authBusy"
      :error-message="authError"
      @submit="handleAuthSubmit"
    />
  </div>
</template>
