<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ChevronLeft, ChevronRight } from "@lucide/vue";

interface Props {
  /** Выбранная дата в виде ISO `YYYY-MM-DD` (без времени). null = ничего не выбрано. */
  value: string | null;
  /** Сегодняшняя дата (для подсветки). По умолчанию — текущий локальный день. */
  today?: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  "update:value": [iso: string];
  pick: [iso: string];
}>();

const RU_WEEKDAYS_SHORT = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"] as const;
const RU_MONTHS_NOM = [
  "Январь",
  "Февраль",
  "Март",
  "Апрель",
  "Май",
  "Июнь",
  "Июль",
  "Август",
  "Сентябрь",
  "Октябрь",
  "Ноябрь",
  "Декабрь",
] as const;

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

function toIso(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function parseIso(iso: string | null): Date | null {
  if (!iso) return null;
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso);
  if (!m) return null;
  return new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
}

const todayDate = computed(() => {
  if (props.today) return parseIso(props.today) ?? new Date();
  return new Date();
});

const selectedDate = computed(() => parseIso(props.value));

function mondayOf(d: Date): Date {
  const out = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const jsDow = out.getDay();
  const mondayOffset = (jsDow + 6) % 7;
  out.setDate(out.getDate() - mondayOffset);
  return out;
}

const viewMonday = ref<Date>(mondayOf(selectedDate.value ?? todayDate.value));

watch(
  () => props.value,
  (iso) => {
    const d = parseIso(iso);
    if (d) viewMonday.value = mondayOf(d);
  },
);

interface DayCell {
  date: Date;
  iso: string;
  weekday: (typeof RU_WEEKDAYS_SHORT)[number];
  isToday: boolean;
  isSelected: boolean;
}

const cells = computed<DayCell[]>(() => {
  const out: DayCell[] = [];
  const todayIso = toIso(todayDate.value);
  const selectedIso = selectedDate.value ? toIso(selectedDate.value) : null;
  for (let i = 0; i < 7; i++) {
    const d = new Date(viewMonday.value);
    d.setDate(d.getDate() + i);
    const iso = toIso(d);
    out.push({
      date: d,
      iso,
      weekday: RU_WEEKDAYS_SHORT[i],
      isToday: iso === todayIso,
      isSelected: iso === selectedIso,
    });
  }
  return out;
});

// Заголовок — месяц и год середины показываемой недели.
// (Если неделя «через месяц» — пишем месяц 4-го дня, т.е. четверга.)
const monthYearLabel = computed(() => {
  const mid = cells.value[3].date;
  return `${RU_MONTHS_NOM[mid.getMonth()]} ${mid.getFullYear()}`;
});

function shiftWeek(delta: number) {
  const next = new Date(viewMonday.value);
  next.setDate(next.getDate() + delta * 7);
  viewMonday.value = next;
}

function pickCell(c: DayCell) {
  emit("update:value", c.iso);
  emit("pick", c.iso);
}
</script>

<template>
  <div
    class="flex flex-col gap-2 bg-[var(--popover,var(--background))] p-4 font-[var(--font-sans,inherit)] text-[var(--popover-foreground,var(--foreground))]"
  >
    <header class="flex items-center justify-between">
      <span class="text-sm font-semibold">{{ monthYearLabel }}</span>
      <div class="flex gap-2">
        <button
          type="button"
          class="inline-flex size-6 items-center justify-center rounded-lg border-0 bg-transparent text-[color-mix(in_srgb,var(--foreground)_60%,transparent)] transition-colors duration-150 ease-[cubic-bezier(0.2,0,0,1)] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-[var(--foreground)]"
          aria-label="Предыдущая неделя"
          @click="shiftWeek(-1)"
        >
          <ChevronLeft :size="14" :stroke-width="1.8" />
        </button>
        <button
          type="button"
          class="inline-flex size-6 items-center justify-center rounded-lg border-0 bg-transparent text-[color-mix(in_srgb,var(--foreground)_60%,transparent)] transition-colors duration-150 ease-[cubic-bezier(0.2,0,0,1)] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-[var(--foreground)]"
          aria-label="Следующая неделя"
          @click="shiftWeek(1)"
        >
          <ChevronRight :size="14" :stroke-width="1.8" />
        </button>
      </div>
    </header>

    <div class="grid grid-cols-7 gap-2">
      <button
        v-for="c in cells"
        :key="c.iso"
        type="button"
        class="flex flex-col items-center justify-center gap-0 rounded-lg border-0 bg-transparent py-2 font-[inherit] text-[var(--foreground)] transition-colors duration-[180ms] ease-[cubic-bezier(0.2,0,0,1)] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)]"
        :class="{
          'outline outline-1 outline-[color-mix(in_srgb,var(--accent)_55%,transparent)]': c.isToday,
          'bg-[var(--accent)] text-[var(--accent-foreground)] hover:bg-[var(--accent)]':
            c.isSelected,
        }"
        @click="pickCell(c)"
      >
        <span
          class="text-[0.625rem] font-semibold uppercase tracking-[0.05em] text-[color-mix(in_srgb,var(--foreground)_55%,transparent)]"
          :class="{
            'text-[color-mix(in_srgb,var(--accent-foreground)_75%,transparent)]': c.isSelected,
          }"
        >
          {{ c.weekday }}
        </span>
        <span
          class="font-[var(--font-mono,ui-monospace,monospace)] text-[0.9375rem] font-medium tabular-nums"
        >
          {{ c.date.getDate() }}
        </span>
      </button>
    </div>
  </div>
</template>
