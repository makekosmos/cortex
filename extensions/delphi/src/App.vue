<script setup lang="ts">
/* eslint-disable no-console */
import { computed, onMounted, onUnmounted, shallowRef, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import {
  fetchProjectsFromArk,
  fetchTasksFromArk,
  getArkApiKey,
  getArkUrl,
  setArkApiKey,
  setArkUrl,
} from "@/services/sync/ark-types";
import { parseConnectionString } from "@/services/sync/pairing";
import type { TodoItem } from "@/types/task";
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
import { useQuickEntry } from "@/composables/useQuickEntry";
import { DesktopChrome, TitlebarHistoryControls, type TitlebarPlatform } from "@kosmos/visuals";
import { useSidebarState } from "@/composables/useSidebarState";
import { arkStatus, connectionState } from "@/composables/useSyncState";

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

const chromePlatform = computed<TitlebarPlatform>(() => {
  if (navigator.platform.startsWith("Mac")) return "mac";
  if (navigator.platform.startsWith("Linux")) return "linux";
  return "windows";
});

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

async function bootstrapElectron() {
  connectionState.value = "syncing";
  arkStatus.value = "connecting";
  store.setHydrated(false);
  store.setTodos([]);
  store.setProjects([]);
  store.setAreas([]);
  store.setTags([]);
  store.setHeadings([]);

  try {
    const todos = (await window.electronAPI?.invoke("ark:listDelphiTasks")) as TodoItem[] | null;
    store.setTodos(Array.isArray(todos) ? todos : []);
    connectionState.value = "offline";
    arkStatus.value = "connected";
  } catch (err) {
    console.warn("[App] Ark task load failed:", err);
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
  setupArkObjectListener();
  if (isElectron) {
    setupCommandBusBridge();
    await bootstrapElectron();
  } else {
    void bootstrapWeb();
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleGlobalKeydown);
  cleanupCommandListeners?.();
  cleanupArkObjectListener?.();
  cleanupArkObjectListener = null;
});
</script>

<template>
  <DesktopChrome appearance="settings" :platform="chromePlatform" class="h-screen w-screen">
    <template #titlebar-leading>
      <TitlebarHistoryControls
        :back-disabled="!canGoBack"
        :forward-disabled="!canGoForward"
        back-title="Назад"
        forward-title="Вперёд"
        @back="navigateBack"
        @forward="navigateForward"
      />
    </template>

    <template #sidebar>
      <SideBar :hidden="sidebarHidden" :show-toggle="false" :reserve-top-inset="false" />
    </template>

    <main class="relative flex min-h-0 min-w-0 flex-1 flex-col">
      <RouterView />
      <QuickEntry />
    </main>
  </DesktopChrome>

  <QuickSearch v-model:open="quickSearchOpen" />

  <AuthOverlay
    v-if="!isElectron && authRequired"
    :busy="authBusy"
    :error-message="authError"
    @submit="handleAuthSubmit"
  />
</template>
