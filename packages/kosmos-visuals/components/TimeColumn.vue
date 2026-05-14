<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";

interface Props {
  /** Текущее выбранное значение (число). */
  value: number;
  /** Минимум (inclusive). */
  min?: number;
  /** Максимум (inclusive). */
  max: number;
  /** Шаг (по умолчанию 1). */
  step?: number;
  /** Aria-label для скриниридера. */
  label?: string;
}

const props = withDefaults(defineProps<Props>(), {
  min: 0,
  step: 1,
  label: undefined,
});

const emit = defineEmits<{
  "update:value": [v: number];
}>();

const items = computed<number[]>(() => {
  const out: number[] = [];
  for (let v = props.min; v <= props.max; v += props.step) out.push(v);
  return out;
});

const root = ref<HTMLElement | null>(null);

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

function scrollSelectedIntoView(behavior: ScrollBehavior = "smooth") {
  if (!root.value) return;
  const el = root.value.querySelector<HTMLElement>(`[data-value="${props.value}"]`);
  if (el) el.scrollIntoView({ block: "center", behavior });
}

onMounted(() => {
  nextTick(() => scrollSelectedIntoView("instant"));
});

watch(
  () => props.value,
  () => scrollSelectedIntoView("smooth"),
);
</script>

<template>
  <div
    ref="root"
    class="kosmos-timecol"
    :aria-label="props.label"
    role="listbox"
  >
    <button
      v-for="v in items"
      :key="v"
      type="button"
      role="option"
      class="kosmos-timecol__item"
      :class="{ 'kosmos-timecol__item--active': v === props.value }"
      :aria-selected="v === props.value"
      :data-value="v"
      @click="emit('update:value', v)"
    >
      {{ pad(v) }}
    </button>
  </div>
</template>

<style scoped>
.kosmos-timecol {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 2px;
  width: 52px;
  height: 200px;
  overflow-y: auto;
  padding: 76px 4px;
  scroll-behavior: smooth;

  /* Кастомный скролл под наши токены. */
  scrollbar-width: thin;
  scrollbar-color: color-mix(in srgb, var(--foreground) 18%, transparent) transparent;
}

.kosmos-timecol::-webkit-scrollbar {
  width: 6px;
}
.kosmos-timecol::-webkit-scrollbar-track {
  background: transparent;
}
.kosmos-timecol::-webkit-scrollbar-thumb {
  background: color-mix(in srgb, var(--foreground) 18%, transparent);
  border-radius: 999px;
  border: 1.5px solid transparent;
  background-clip: padding-box;
}
.kosmos-timecol::-webkit-scrollbar-thumb:hover {
  background: color-mix(in srgb, var(--foreground) 35%, transparent);
  background-clip: padding-box;
}

.kosmos-timecol__item {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 32px;
  flex-shrink: 0;
  background: transparent;
  border: none;
  border-radius: 6px;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  font-size: 0.875rem;
  font-variant-numeric: tabular-nums;
  font-family: var(--font-mono, ui-monospace, monospace);
  cursor: pointer;
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-timecol__item:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.kosmos-timecol__item--active {
  background: var(--accent);
  color: var(--accent-foreground);
  font-weight: 600;
}

.kosmos-timecol__item--active:hover {
  background: var(--accent);
  color: var(--accent-foreground);
}
</style>
