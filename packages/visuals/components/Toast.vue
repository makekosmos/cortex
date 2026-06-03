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
  <div
    class="relative flex max-w-80 items-start gap-2 overflow-hidden rounded-lg border border-[var(--border)] bg-[color-mix(in_srgb,var(--background)_92%,transparent)] px-4 py-2 text-sm leading-[1.35] text-[var(--foreground)] shadow-[0_4px_16px_color-mix(in_srgb,var(--foreground)_12%,transparent)] backdrop-blur-sm pointer-events-auto"
    :class="{
      'border-[color-mix(in_srgb,var(--accent)_55%,var(--border))]': tone === 'success',
      'border-[color-mix(in_srgb,var(--destructive)_55%,var(--border))] text-[color-mix(in_srgb,var(--destructive)_85%,var(--foreground))]':
        tone === 'error',
    }"
    role="status"
  >
    <div v-if="loading" class="kosmos-toast__spinner" aria-hidden="true" />
    <div class="min-w-0">
      <div v-if="title" class="mb-2 text-[0.82rem] font-[650]">{{ title }}</div>
      <div class="[overflow-wrap:anywhere]">{{ message }}</div>
      <div
        v-if="description"
        class="mt-2 text-[0.76rem] text-[color-mix(in_srgb,var(--foreground)_58%,transparent)]"
      >
        {{ description }}
      </div>
    </div>
    <button
      v-if="closable"
      type="button"
      class="ml-2 inline-flex size-6 items-center justify-center rounded-lg border-0 bg-transparent p-0 text-base leading-none text-[color-mix(in_srgb,var(--foreground)_58%,transparent)] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-[var(--foreground)]"
      aria-label="Закрыть уведомление"
      @click="emit('dismiss')"
    >
      ×
    </button>
    <div v-if="loading" class="kosmos-toast__progress" aria-hidden="true" />
  </div>
</template>

<style scoped>
.kosmos-toast__spinner {
  width: 16px;
  height: 16px;
  margin-top: 0;
  flex-shrink: 0;
  border-radius: 999px;
  border: 2px solid color-mix(in srgb, var(--foreground) 18%, transparent);
  border-top-color: color-mix(in srgb, var(--accent) 82%, var(--foreground));
  animation: kosmos-toast-spin 800ms linear infinite;
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
