<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { Copy, Minus, Square, X } from "@lucide/vue";

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
  <div class="kosmos-window-controls inline-flex items-center gap-2">
    <button
      v-if="!hideMinimize"
      type="button"
      class="kosmos-window-controls__button inline-flex size-[var(--kosmos-titlebar-control-size,32px)] items-center justify-center rounded-[var(--kosmos-titlebar-control-radius,8px)] border-0 bg-transparent p-0 text-[color-mix(in_srgb,var(--sidebar-foreground)_72%,transparent)] transition-[background-color,color] duration-[120ms] ease-in hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-(--foreground)"
      title="Свернуть"
      aria-label="Свернуть"
      data-testid="window-control-minimize"
      @click="handleMinimize"
    >
      <Minus :size="16" :stroke-width="2" />
    </button>

    <button
      v-if="!hideMaximize"
      type="button"
      class="kosmos-window-controls__button inline-flex size-[var(--kosmos-titlebar-control-size,32px)] items-center justify-center rounded-[var(--kosmos-titlebar-control-radius,8px)] border-0 bg-transparent p-0 text-[color-mix(in_srgb,var(--sidebar-foreground)_72%,transparent)] transition-[background-color,color] duration-[120ms] ease-in hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-(--foreground)"
      :title="isMaximized ? 'Свернуть в окно' : 'Развернуть'"
      :aria-label="isMaximized ? 'Свернуть в окно' : 'Развернуть'"
      data-testid="window-control-maximize"
      @click="handleMaximize"
    >
      <Copy v-if="isMaximized" :size="16" :stroke-width="2" class="scale-x-[-1]" />
      <Square v-else :size="16" :stroke-width="2" />
    </button>

    <button
      v-if="!hideClose"
      type="button"
      class="kosmos-window-controls__button inline-flex size-[var(--kosmos-titlebar-control-size,32px)] items-center justify-center rounded-[var(--kosmos-titlebar-control-radius,8px)] border-0 bg-transparent p-0 text-[color-mix(in_srgb,var(--sidebar-foreground)_72%,transparent)] transition-[background-color,color] duration-[120ms] ease-in hover:bg-[var(--destructive)] hover:text-[var(--destructive-foreground)]"
      title="Закрыть"
      aria-label="Закрыть"
      data-testid="window-control-close"
      @click="handleClose"
    >
      <X :size="16" :stroke-width="2" />
    </button>
  </div>
</template>

<style scoped>
.kosmos-window-controls {
  -webkit-app-region: no-drag;
}

.kosmos-window-controls__button {
  -webkit-app-region: no-drag;
}
</style>
