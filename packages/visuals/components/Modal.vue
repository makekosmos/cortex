<script setup lang="ts">
import { onBeforeUnmount, watch } from "vue";

interface Props {
  open: boolean;
  title?: string;
  /** Ширина модалки в CSS (по умолчанию `min(440px, 92vw)`). */
  width?: string;
  /** Закрытие по клику вне (на backdrop). По умолчанию true. */
  closeOnBackdrop?: boolean;
  /** Скрыть × кнопку в header'е (если у модалки есть явная "Отмена" в footer'е). */
  hideClose?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  title: undefined,
  width: undefined,
  closeOnBackdrop: true,
  hideClose: false,
});

const emit = defineEmits<{
  close: [];
}>();

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && props.open) emit("close");
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      document.addEventListener("keydown", onKey);
    } else {
      document.removeEventListener("keydown", onKey);
    }
  },
);

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKey);
});

function onBackdropPointerDown(e: PointerEvent) {
  if (!props.closeOnBackdrop) return;
  if (e.target === e.currentTarget) emit("close");
}
</script>

<template>
  <Teleport to="body">
    <transition name="kosmos-modal">
      <div
        v-if="props.open"
        class="fixed inset-0 z-[9000] flex items-center justify-center bg-[color-mix(in_srgb,var(--background)_48%,transparent)] p-4"
        role="presentation"
        @pointerdown="onBackdropPointerDown"
      >
        <div
          class="kosmos-modal__panel flex max-h-[calc(100vh-2rem)] flex-col overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--popover,var(--background))] text-[var(--popover-foreground,var(--foreground))] shadow-[var(--shadow-floating)] [corner-shape:var(--corner-shape)]"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="props.title ? 'kosmos-modal-title' : undefined"
          :style="{ width: props.width ?? 'min(440px, 92vw)' }"
        >
          <header
            v-if="props.title || $slots.header"
            class="flex items-center justify-between gap-4 border-b border-[var(--border)] px-4 py-4"
          >
            <slot name="header">
              <h2
                id="kosmos-modal-title"
                class="m-0 text-[0.95rem] font-semibold text-[var(--foreground)]"
              >
                {{ props.title }}
              </h2>
            </slot>
            <button
              v-if="!props.hideClose"
              type="button"
              class="inline-flex size-6 items-center justify-center rounded-lg border-0 bg-transparent text-lg leading-none text-[color-mix(in_srgb,var(--foreground)_60%,transparent)] hover:bg-[color-mix(in_srgb,var(--foreground)_10%,transparent)] hover:text-[var(--foreground)]"
              aria-label="Закрыть"
              @click="emit('close')"
            >
              ×
            </button>
          </header>
          <div class="min-h-0 flex-1 overflow-y-auto p-4">
            <slot />
          </div>
          <footer
            v-if="$slots.footer"
            class="flex items-center justify-end gap-2 border-t border-[var(--border)] px-4 py-4"
          >
            <slot name="footer" />
          </footer>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.kosmos-modal-enter-active,
.kosmos-modal-leave-active {
  transition: opacity 140ms cubic-bezier(0.2, 0, 0, 1);
}
.kosmos-modal-enter-active .kosmos-modal__panel,
.kosmos-modal-leave-active .kosmos-modal__panel {
  transition: transform 180ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-modal-enter-from,
.kosmos-modal-leave-to {
  opacity: 0;
}

.kosmos-modal-enter-from .kosmos-modal__panel,
.kosmos-modal-leave-to .kosmos-modal__panel {
  transform: translateY(8px) scale(0.98);
}
</style>
