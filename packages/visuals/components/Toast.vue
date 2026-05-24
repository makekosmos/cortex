<script setup lang="ts">
import type { ToastTone } from "../composables/useToast";

interface Props {
  message: string;
  title?: string;
  description?: string;
  tone?: ToastTone;
  loading?: boolean;
  closable?: boolean;
}

withDefaults(defineProps<Props>(), {
  tone: "info",
  loading: false,
  closable: false,
});

const emit = defineEmits<{
  dismiss: [];
}>();
</script>

<template>
  <!-- Regression L6 (2026-05-24): aria-live lives on ToastHost (the polite
       live region). Double aria-live on a child inside a live region causes
       screen readers to announce twice. role="status" is kept for semantics. -->
  <div class="kosmos-toast" :class="`kosmos-toast--${tone}`" role="status">
    <div v-if="loading" class="kosmos-toast__spinner" aria-hidden="true" />
    <div class="kosmos-toast__body">
      <div v-if="title" class="kosmos-toast__title">{{ title }}</div>
      <div class="kosmos-toast__message">{{ message }}</div>
      <div v-if="description" class="kosmos-toast__description">{{ description }}</div>
    </div>
    <button
      v-if="closable"
      type="button"
      class="kosmos-toast__close"
      aria-label="Закрыть уведомление"
      @click="emit('dismiss')"
    >
      ×
    </button>
    <div v-if="loading" class="kosmos-toast__progress" aria-hidden="true" />
  </div>
</template>

<style scoped>
.kosmos-toast {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 0.5rem 0.875rem;
  border-radius: 8px;
  font-size: 0.875rem;
  line-height: 1.35;
  color: var(--foreground);
  background: color-mix(in srgb, var(--background) 92%, transparent);
  border: 1px solid var(--border);
  box-shadow: 0 4px 16px color-mix(in srgb, var(--foreground) 12%, transparent);
  backdrop-filter: blur(8px);
  max-width: 320px;
  pointer-events: auto;
  overflow: hidden;
}

.kosmos-toast__body {
  min-width: 0;
}

.kosmos-toast__title {
  margin-bottom: 2px;
  font-size: 0.82rem;
  font-weight: 650;
}

.kosmos-toast__message {
  overflow-wrap: anywhere;
}

.kosmos-toast__description {
  margin-top: 2px;
  color: color-mix(in srgb, var(--foreground) 58%, transparent);
  font-size: 0.76rem;
}

.kosmos-toast__spinner {
  width: 14px;
  height: 14px;
  margin-top: 2px;
  flex-shrink: 0;
  border-radius: 999px;
  border: 2px solid color-mix(in srgb, var(--foreground) 18%, transparent);
  border-top-color: color-mix(in srgb, var(--accent) 82%, var(--foreground));
  animation: kosmos-toast-spin 800ms linear infinite;
}

.kosmos-toast__close {
  margin: -3px -5px 0 4px;
  padding: 0 4px;
  border: none;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 58%, transparent);
  cursor: pointer;
  font: inherit;
  font-size: 16px;
  line-height: 1;
}

.kosmos-toast__close:hover {
  color: var(--foreground);
}

.kosmos-toast__progress {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: 2px;
  background: linear-gradient(
    90deg,
    transparent,
    color-mix(in srgb, var(--accent) 82%, var(--foreground)),
    transparent
  );
  animation: kosmos-toast-progress 1.2s ease-in-out infinite;
}

.kosmos-toast--success {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
}

.kosmos-toast--error {
  border-color: color-mix(in srgb, #d54a4a 55%, var(--border));
  color: color-mix(in srgb, #d54a4a 85%, var(--foreground));
}

@keyframes kosmos-toast-spin {
  to {
    transform: rotate(360deg);
  }
}

@keyframes kosmos-toast-progress {
  0% {
    transform: translateX(-70%);
  }
  100% {
    transform: translateX(70%);
  }
}
</style>
