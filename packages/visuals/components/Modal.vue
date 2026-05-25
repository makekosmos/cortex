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
        class="kosmos-modal__backdrop"
        role="presentation"
        @pointerdown="onBackdropPointerDown"
      >
        <div
          class="kosmos-modal__panel"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="props.title ? 'kosmos-modal-title' : undefined"
          :style="{ width: props.width ?? 'min(440px, 92vw)' }"
        >
          <header v-if="props.title || $slots.header" class="kosmos-modal__head">
            <slot name="header">
              <h2 id="kosmos-modal-title" class="kosmos-modal__title">{{ props.title }}</h2>
            </slot>
            <button
              v-if="!props.hideClose"
              type="button"
              class="kosmos-modal__close"
              aria-label="Закрыть"
              @click="emit('close')"
            >
              ×
            </button>
          </header>
          <div class="kosmos-modal__body">
            <slot />
          </div>
          <footer v-if="$slots.footer" class="kosmos-modal__foot">
            <slot name="footer" />
          </footer>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.kosmos-modal__backdrop {
  position: fixed;
  inset: 0;
  z-index: 9000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
  background: rgb(0 0 0 / 48%);
}

.kosmos-modal__panel {
  display: flex;
  flex-direction: column;
  max-height: calc(100vh - 2rem);
  background: var(--popover, var(--background));
  color: var(--popover-foreground, var(--foreground));
  border: 1px solid var(--border);
  border-radius: var(--radius);
  corner-shape: var(--corner-shape);
  box-shadow:
    0 24px 64px rgb(0 0 0 / 38%),
    0 8px 24px rgb(0 0 0 / 18%);
  overflow: hidden;
}

.kosmos-modal__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.875rem 1rem 0.75rem;
  border-bottom: 1px solid var(--border);
}

.kosmos-modal__title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--foreground);
}

.kosmos-modal__close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  font-size: 18px;
  line-height: 1;
  background: transparent;
  border: none;
  border-radius: 6px;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
}

.kosmos-modal__close:hover {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
  color: var(--foreground);
}

.kosmos-modal__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 1rem;
}

.kosmos-modal__foot {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.5rem;
  padding: 0.75rem 1rem;
  border-top: 1px solid var(--border);
}

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
