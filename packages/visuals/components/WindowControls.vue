<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { Copy, Minus, Square, X } from "lucide-vue-next";

interface Props {
  /** Скрыть кнопку minimize. */
  hideMinimize?: boolean;
  /** Скрыть кнопку maximize/restore. */
  hideMaximize?: boolean;
  /** Скрыть кнопку close. */
  hideClose?: boolean;
}

withDefaults(defineProps<Props>(), {
  hideMinimize: false,
  hideMaximize: false,
  hideClose: false,
});

interface KeplerWindowApi {
  close: () => Promise<void>;
  minimize: () => Promise<void>;
  maximize: () => Promise<void>;
  isMaximized?: () => Promise<boolean>;
  onMaximizedChange?: (handler: (value: boolean) => void) => () => void;
}

function getApi(): KeplerWindowApi | null {
  if (typeof window === "undefined") return null;
  const kepler = (window as unknown as { kepler?: { window?: KeplerWindowApi } }).kepler;
  return kepler?.window ?? null;
}

const isMaximized = ref(false);
let unsubscribe: (() => void) | null = null;

onMounted(async () => {
  const api = getApi();
  if (!api) return;
  if (api.isMaximized) {
    try {
      isMaximized.value = await api.isMaximized();
    } catch {
      // ignore — non-extension context (no IPC)
    }
  }
  if (api.onMaximizedChange) {
    unsubscribe = api.onMaximizedChange((value) => {
      isMaximized.value = value;
    });
  }
});

onBeforeUnmount(() => {
  if (unsubscribe) {
    try {
      unsubscribe();
    } catch {
      // ignore
    }
    unsubscribe = null;
  }
});

function handleMinimize(): void {
  void getApi()?.minimize();
}

function handleMaximize(): void {
  void getApi()?.maximize();
}

function handleClose(): void {
  void getApi()?.close();
}
</script>

<template>
  <div class="kosmos-window-controls">
    <button
      v-if="!hideMinimize"
      type="button"
      class="kosmos-window-controls__button"
      title="Свернуть"
      aria-label="Свернуть"
      data-testid="window-control-minimize"
      @click="handleMinimize"
    >
      <Minus :size="14" :stroke-width="2" />
    </button>

    <button
      v-if="!hideMaximize"
      type="button"
      class="kosmos-window-controls__button"
      :title="isMaximized ? 'Свернуть в окно' : 'Развернуть'"
      :aria-label="isMaximized ? 'Свернуть в окно' : 'Развернуть'"
      data-testid="window-control-maximize"
      @click="handleMaximize"
    >
      <Copy
        v-if="isMaximized"
        :size="12"
        :stroke-width="2"
        class="kosmos-window-controls__restore-icon"
      />
      <Square v-else :size="12" :stroke-width="2" />
    </button>

    <button
      v-if="!hideClose"
      type="button"
      class="kosmos-window-controls__button kosmos-window-controls__button--danger"
      title="Закрыть"
      aria-label="Закрыть"
      data-testid="window-control-close"
      @click="handleClose"
    >
      <X :size="14" :stroke-width="2" />
    </button>
  </div>
</template>

<style scoped>
.kosmos-window-controls {
  display: inline-flex;
  align-items: center;
  gap: 0.125rem;
  -webkit-app-region: no-drag;
}

.kosmos-window-controls__button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--kosmos-titlebar-control-size, 32px);
  height: var(--kosmos-titlebar-control-size, 32px);
  border-radius: var(--kosmos-titlebar-control-radius, 10px);
  background: transparent;
  border: none;
  padding: 0;
  cursor: pointer;
  color: color-mix(in srgb, var(--sidebar-foreground) 72%, transparent);
  transition:
    background-color 120ms ease,
    color 120ms ease;
  -webkit-app-region: no-drag;
}

.kosmos-window-controls__button:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.kosmos-window-controls__button--danger:hover {
  background: #c42b1c;
  color: #ffffff;
}

.kosmos-window-controls__restore-icon {
  /* Lucide Copy визуально имеет смещение «верхнего» rect'а — для restore
     иконки это семантически правильно, но мы зеркалим горизонтально, чтобы
     стиль совпадал с Windows 11 (передний rect снизу-слева, задний — сверху-справа). */
  transform: scaleX(-1);
}
</style>
