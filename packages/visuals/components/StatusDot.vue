<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { StatusDotTone } from "./types";

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

const toneClass = computed(() => {
  switch (props.tone) {
    case "success":
      return "text-[var(--status-success)]";
    case "warning":
      return "text-[var(--status-warning)]";
    case "danger":
      return "text-[var(--destructive)]";
    case "neutral":
    default:
      return "text-[color-mix(in_srgb,var(--muted-foreground)_72%,transparent)]";
  }
});

const normalizedSideOffset = computed(() => Math.max(0, Math.round(props.sideOffset / 8) * 8));

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
  <div ref="rootRef" class="relative inline-flex items-center justify-center">
    <button
      type="button"
      class="inline-flex size-8 items-center justify-center rounded-lg transition-[background-color,color] duration-150 ease-out [corner-shape:var(--corner-shape)] hover:bg-[color-mix(in_srgb,var(--sidebar-foreground)_8%,transparent)]"
      :class="toneClass"
      :title="label"
      :aria-label="label"
      :aria-expanded="open"
      @click="toggle()"
    >
      <span class="size-2 rounded-full bg-current" />
    </button>

    <div
      v-if="open"
      class="absolute right-0 z-[80] min-w-56 max-w-80 rounded-lg border border-[var(--border)] bg-[var(--popover,var(--background))] p-4 text-[var(--popover-foreground,var(--foreground))] shadow-[0_8px_24px_color-mix(in_srgb,var(--foreground)_22%,transparent),0_2px_8px_color-mix(in_srgb,var(--foreground)_12%,transparent)]"
      :style="{ top: `calc(100% + ${normalizedSideOffset}px)` }"
      role="dialog"
      :aria-label="label"
    >
      <slot>
        <p class="m-0 text-[0.8125rem] leading-[1.45]">{{ label }}</p>
      </slot>
    </div>
  </div>
</template>
