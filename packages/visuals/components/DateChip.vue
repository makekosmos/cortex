<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, shallowRef } from "vue";
import { Calendar as CalendarIcon, X } from "@lucide/vue";
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
  <div ref="anchorRef" class="relative inline-flex">
    <!--
      Composite chip = 2 sibling button'а в одном flex-контейнере. Раньше
      clear был `<span role="button">` внутри `<button>` — невалидный
      nested interactive, screen reader'ы collapse'или в одну кнопку и
      «Очистить дату» становилась недоступна с клавиатуры. Теперь две
      нормальные кнопки, контейнер только визуально объединяет их.
    -->
    <div
      :class="[
        'inline-flex items-center rounded-lg bg-[var(--secondary)] text-xs text-[color-mix(in_srgb,var(--foreground)_65%,transparent)] transition-colors duration-[120ms] ease-[cubic-bezier(0.2,0,0,1)] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,var(--secondary))] hover:text-[var(--foreground)]',
        props.value
          ? 'bg-[color-mix(in_srgb,var(--accent)_18%,transparent)] text-[var(--accent)] hover:bg-[color-mix(in_srgb,var(--accent)_26%,transparent)]'
          : '',
      ]"
    >
      <button
        type="button"
        class="inline-flex items-center gap-2 rounded-[inherit] border-0 bg-transparent px-4 py-2 text-[inherit] text-inherit"
        :class="{ 'px-2 py-0 text-[0.7rem]': compact }"
        @click="toggle"
      >
        <CalendarIcon :size="compact ? 11 : 12" class="shrink-0 text-current" />
        <span class="leading-none">{{ label }}</span>
      </button>
      <button
        v-if="props.value"
        type="button"
        class="mr-2 inline-flex size-4 items-center justify-center rounded-lg border-0 bg-transparent text-[color-mix(in_srgb,currentColor_70%,transparent)] hover:bg-[color-mix(in_srgb,currentColor_18%,transparent)] hover:text-current focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[var(--accent,currentColor)]"
        :class="{ 'mr-2': compact }"
        aria-label="Очистить дату"
        @click.stop="clear"
      >
        <X :size="10" />
      </button>
    </div>

    <div
      v-if="open"
      class="absolute left-0 top-[calc(100%+8px)] z-[60] min-w-80 overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--popover,var(--background))] shadow-[var(--shadow-floating)]"
    >
      <Calendar :value="props.value" :today="todayIso" @pick="pick" />
    </div>
  </div>
</template>
