<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";

export type StatusDotTone = "success" | "warning" | "danger" | "neutral";

interface Props {
  tone?: StatusDotTone;
  label: string;
  sideOffset?: number;
}

const props = withDefaults(defineProps<Props>(), {
  tone: "neutral",
  sideOffset: 8,
});

const rootRef = ref<HTMLElement | null>(null);
const open = ref(false);

const buttonClasses = computed(() => [
  "kosmos-status-dot",
  `kosmos-status-dot--${props.tone}`,
]);

function toggle() {
  open.value = !open.value;
}

function close() {
  open.value = false;
}

function handleDocumentPointerDown(event: PointerEvent) {
  if (!open.value || !rootRef.value) {
    return;
  }

  const target = event.target;
  if (target instanceof Node && rootRef.value.contains(target)) {
    return;
  }

  close();
}

function handleDocumentKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    close();
  }
}

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown);
  document.addEventListener("keydown", handleDocumentKeydown);
});

onUnmounted(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
  document.removeEventListener("keydown", handleDocumentKeydown);
});
</script>

<template>
  <div ref="rootRef" class="kosmos-status-dot-anchor">
    <button
      type="button"
      :class="buttonClasses"
      :title="label"
      :aria-label="label"
      :aria-expanded="open"
      @click="toggle()"
    >
      <span class="kosmos-status-dot__core" />
    </button>

    <div
      v-if="open"
      class="kosmos-status-dot__popover"
      :style="{ top: `calc(100% + ${props.sideOffset}px)` }"
      role="dialog"
      :aria-label="label"
    >
      <slot>
        <p class="kosmos-status-dot__label">{{ label }}</p>
      </slot>
    </div>
  </div>
</template>

<style scoped>
.kosmos-status-dot-anchor {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.kosmos-status-dot {
  --kosmos-status-dot-color: color-mix(in srgb, var(--muted-foreground) 75%, transparent);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: calc(var(--radius) * 0.9);
  corner-shape: var(--corner-shape);
  color: var(--kosmos-status-dot-color);
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-status-dot:hover {
  background: color-mix(in srgb, var(--sidebar-foreground) 8%, transparent);
}

.kosmos-status-dot__core {
  width: 10px;
  height: 10px;
  border-radius: 999px;
  background: currentColor;
}

.kosmos-status-dot--success {
  --kosmos-status-dot-color: var(--status-success);
}

.kosmos-status-dot--warning {
  --kosmos-status-dot-color: #c59f4d;
}

.kosmos-status-dot--danger {
  --kosmos-status-dot-color: #c87373;
}

.kosmos-status-dot--neutral {
  --kosmos-status-dot-color: color-mix(in srgb, var(--muted-foreground) 72%, transparent);
}

.kosmos-status-dot__popover {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  z-index: 80;
  min-width: 220px;
  max-width: 320px;
  padding: 0.75rem 0.85rem;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--popover, var(--background));
  color: var(--popover-foreground, var(--foreground));
  box-shadow:
    0 8px 24px color-mix(in srgb, #000 22%, transparent),
    0 2px 8px color-mix(in srgb, #000 12%, transparent);
}

.kosmos-status-dot__label {
  margin: 0;
  font-size: 0.8125rem;
  line-height: 1.45;
}
</style>
