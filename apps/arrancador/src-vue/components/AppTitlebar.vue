<script setup lang="ts">
import { Titlebar, TitlebarHistoryControls } from "@kepler/visuals";
import { Maximize2, Minimize2, PanelLeft, X } from "lucide-vue-next";
import { computed, onMounted, onUnmounted, shallowRef } from "vue";
import {
  closeWindow,
  getFallbackWindowChromePlatform,
  minimizeWindow,
  toggleMaximizeWindow,
} from "../../src/lib/window-chrome";
import { useLanguage } from "../composables/useLanguage";

type WindowControlsOverlayLike = {
  visible?: boolean;
  addEventListener?: (
    type: "geometrychange",
    listener: EventListenerOrEventListenerObject,
  ) => void;
  removeEventListener?: (
    type: "geometrychange",
    listener: EventListenerOrEventListenerObject,
  ) => void;
};

const props = defineProps<{
  sidebarHidden: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
}>();

const emit = defineEmits<{
  toggleSidebar: [];
  back: [];
  forward: [];
}>();

const { language } = useLanguage();

const platform = getFallbackWindowChromePlatform();
const hasNativeWindowControls = shallowRef(false);

const sidebarLabel = computed(() =>
  language.value === "ru"
    ? props.sidebarHidden
      ? "Показать боковую панель"
      : "Скрыть боковую панель"
    : props.sidebarHidden
      ? "Show sidebar"
      : "Hide sidebar",
);

const controlsCopy = computed(() =>
  language.value === "ru"
    ? {
        back: "Назад",
        forward: "Вперёд",
        minimize: "Свернуть окно",
        maximize: "Развернуть окно",
        close: "Закрыть окно",
      }
    : {
        back: "Back",
        forward: "Forward",
        minimize: "Minimize window",
        maximize: "Maximize window",
        close: "Close window",
      },
);

const showFallbackWindowControls = computed(
  () => platform === "windows" && !hasNativeWindowControls.value,
);

function getWindowControlsOverlayVisible() {
  if (typeof navigator === "undefined") {
    return false;
  }

  const overlay = (
    navigator as Navigator & {
      windowControlsOverlay?: WindowControlsOverlayLike;
    }
  ).windowControlsOverlay;

  return Boolean(overlay?.visible);
}

let overlayCleanup: (() => void) | null = null;

onMounted(() => {
  hasNativeWindowControls.value = getWindowControlsOverlayVisible();

  if (typeof navigator === "undefined") {
    return;
  }

  const overlay = (
    navigator as Navigator & {
      windowControlsOverlay?: WindowControlsOverlayLike;
    }
  ).windowControlsOverlay;

  if (!overlay?.addEventListener || !overlay.removeEventListener) {
    hasNativeWindowControls.value = false;
    return;
  }

  const syncOverlayState = () => {
    hasNativeWindowControls.value = Boolean(overlay.visible);
  };

  syncOverlayState();
  overlay.addEventListener("geometrychange", syncOverlayState);
  overlayCleanup = () =>
    overlay.removeEventListener?.("geometrychange", syncOverlayState);
});

onUnmounted(() => {
  overlayCleanup?.();
  overlayCleanup = null;
});
</script>

<template>
  <Titlebar
    :platform="platform"
    :class="{
      'app-titlebar--mac': platform === 'mac',
      'app-titlebar--windows': platform === 'windows',
    }"
  >
    <template #leading>
      <button
        type="button"
        class="arrancador-titlebar-button"
        :title="sidebarLabel"
        :aria-label="sidebarLabel"
        @click="emit('toggleSidebar')"
      >
        <PanelLeft class="h-4 w-4" />
      </button>

      <TitlebarHistoryControls
        :back-disabled="!canGoBack"
        :forward-disabled="!canGoForward"
        :back-title="controlsCopy.back"
        :forward-title="controlsCopy.forward"
        @back="emit('back')"
        @forward="emit('forward')"
      />
    </template>

    <template #center />

    <template #trailing>
      <div v-if="showFallbackWindowControls" class="arrancador-window-controls">
        <button
          type="button"
          class="arrancador-window-controls__button"
          :title="controlsCopy.minimize"
          :aria-label="controlsCopy.minimize"
          @click="void minimizeWindow()"
        >
          <Minimize2 class="h-4 w-4" />
        </button>
        <button
          type="button"
          class="arrancador-window-controls__button"
          :title="controlsCopy.maximize"
          :aria-label="controlsCopy.maximize"
          @click="void toggleMaximizeWindow()"
        >
          <Maximize2 class="h-4 w-4" />
        </button>
        <button
          type="button"
          class="arrancador-window-controls__button arrancador-window-controls__button--danger"
          :title="controlsCopy.close"
          :aria-label="controlsCopy.close"
          @click="void closeWindow()"
        >
          <X class="h-4 w-4" />
        </button>
      </div>
    </template>
  </Titlebar>
</template>
