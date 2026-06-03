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
    class="kosmos-timecol flex h-[200px] w-16 flex-col items-stretch gap-0 overflow-y-auto px-2 py-20 scroll-smooth"
    :aria-label="props.label"
    role="listbox"
  >
    <button
      v-for="v in items"
      :key="v"
      type="button"
      role="option"
      class="inline-flex h-8 shrink-0 items-center justify-center rounded-lg border-0 bg-transparent font-[var(--font-mono,ui-monospace,monospace)] text-sm tabular-nums text-[color-mix(in_srgb,var(--foreground)_55%,transparent)] transition-colors duration-[120ms] ease-[cubic-bezier(0.2,0,0,1)] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-[var(--foreground)]"
      :class="{
        'bg-[var(--accent)] font-semibold text-[var(--accent-foreground)] hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]':
          v === props.value,
      }"
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
</style>
