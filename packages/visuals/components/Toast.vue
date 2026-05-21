<script setup lang="ts">
import type { ToastTone } from "../composables/useToast";

interface Props {
  message: string;
  tone?: ToastTone;
}

withDefaults(defineProps<Props>(), {
  tone: "info",
});
</script>

<template>
  <div class="kosmos-toast" :class="`kosmos-toast--${tone}`" role="status" aria-live="polite">
    {{ message }}
  </div>
</template>

<style scoped>
.kosmos-toast {
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
}

.kosmos-toast--success {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
}

.kosmos-toast--error {
  border-color: color-mix(in srgb, #d54a4a 55%, var(--border));
  color: color-mix(in srgb, #d54a4a 85%, var(--foreground));
}
</style>
