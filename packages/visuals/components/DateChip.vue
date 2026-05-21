<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, shallowRef } from "vue";
import { Calendar as CalendarIcon, X } from "lucide-vue-next";
import Calendar from "./Calendar.vue";

interface Props {
  /** ISO `YYYY-MM-DD` или null. */
  value: string | null;
  /** Текст когда дата не выбрана. */
  placeholder?: string;
  /** Если true — компактный размер чипа. */
  compact?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: "Дата",
  compact: false,
});

const emit = defineEmits<{
  "update:value": [iso: string | null];
}>();

const open = shallowRef(false);
const anchorRef = ref<HTMLElement | null>(null);

const RU_MONTHS_SHORT = [
  "янв",
  "фев",
  "мар",
  "апр",
  "май",
  "июн",
  "июл",
  "авг",
  "сен",
  "окт",
  "ноя",
  "дек",
] as const;

const label = computed(() => {
  if (!props.value) return props.placeholder;
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(props.value);
  if (!m) return props.value;
  const day = Number(m[3]);
  const month = RU_MONTHS_SHORT[Number(m[2]) - 1];
  return `${day} ${month}`;
});

const todayIso = computed(() => {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
});

function pick(iso: string) {
  emit("update:value", iso);
  open.value = false;
}

function clear(e: MouseEvent) {
  e.stopPropagation();
  emit("update:value", null);
}

function toggle() {
  open.value = !open.value;
}

function onDocPointerDown(e: PointerEvent) {
  if (!open.value || !anchorRef.value) return;
  if (anchorRef.value.contains(e.target as Node)) return;
  open.value = false;
}

function onDocKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape" && open.value) open.value = false;
}

onMounted(() => {
  document.addEventListener("pointerdown", onDocPointerDown);
  document.addEventListener("keydown", onDocKeyDown);
});

onUnmounted(() => {
  document.removeEventListener("pointerdown", onDocPointerDown);
  document.removeEventListener("keydown", onDocKeyDown);
});
</script>

<template>
  <div ref="anchorRef" class="kosmos-datechip-anchor">
    <!--
      Composite chip = 2 sibling button'а в одном flex-контейнере. Раньше
      clear был `<span role="button">` внутри `<button>` — невалидный
      nested interactive, screen reader'ы collapse'или в одну кнопку и
      «Очистить дату» становилась недоступна с клавиатуры. Теперь две
      нормальные кнопки, контейнер только визуально объединяет их.
    -->
    <div
      :class="[
        'kosmos-datechip',
        props.value ? 'kosmos-datechip--active' : '',
        compact ? 'kosmos-datechip--compact' : '',
      ]"
    >
      <button type="button" class="kosmos-datechip__toggle" @click="toggle">
        <CalendarIcon :size="compact ? 11 : 12" class="kosmos-datechip__icon" />
        <span class="kosmos-datechip__label">{{ label }}</span>
      </button>
      <button
        v-if="props.value"
        type="button"
        class="kosmos-datechip__clear"
        aria-label="Очистить дату"
        @click.stop="clear"
      >
        <X :size="10" />
      </button>
    </div>

    <div v-if="open" class="kosmos-datechip__popover">
      <Calendar :value="props.value" :today="todayIso" @pick="pick" />
    </div>
  </div>
</template>

<style scoped>
.kosmos-datechip-anchor {
  position: relative;
  display: inline-flex;
}

.kosmos-datechip {
  display: inline-flex;
  align-items: center;
  border-radius: 999px;
  background: var(--secondary);
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  font-size: 0.75rem;
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-datechip:hover {
  background: color-mix(in srgb, var(--foreground) 8%, var(--secondary));
  color: var(--foreground);
}

.kosmos-datechip--active {
  background: color-mix(in srgb, var(--accent) 18%, transparent);
  color: var(--accent);
}

.kosmos-datechip--active:hover {
  background: color-mix(in srgb, var(--accent) 26%, transparent);
}

.kosmos-datechip__toggle {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  padding: 0.375rem 0.75rem;
  border: none;
  background: transparent;
  color: inherit;
  font-size: inherit;
  border-radius: inherit;
  cursor: pointer;
}

.kosmos-datechip--compact .kosmos-datechip__toggle {
  padding: 0.25rem 0.55rem;
  font-size: 0.7rem;
}

.kosmos-datechip__icon {
  flex-shrink: 0;
  color: currentColor;
}

.kosmos-datechip__label {
  line-height: 1;
}

.kosmos-datechip__clear {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  margin-right: 0.5rem;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: color-mix(in srgb, currentColor 70%, transparent);
  cursor: pointer;
}

.kosmos-datechip--compact .kosmos-datechip__clear {
  margin-right: 0.35rem;
}

.kosmos-datechip__clear:hover {
  background: color-mix(in srgb, currentColor 18%, transparent);
  color: currentColor;
}

.kosmos-datechip__clear:focus-visible {
  outline: 2px solid var(--accent, currentColor);
  outline-offset: 1px;
}

.kosmos-datechip__popover {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 60;
  min-width: 320px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--popover, var(--background));
  box-shadow:
    0 10px 32px color-mix(in srgb, #000 28%, transparent),
    0 3px 10px color-mix(in srgb, #000 14%, transparent);
  overflow: hidden;
}
</style>
