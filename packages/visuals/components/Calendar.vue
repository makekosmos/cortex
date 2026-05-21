<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ChevronLeft, ChevronRight } from "lucide-vue-next";

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
  <div class="kosmos-calendar">
    <header class="kosmos-calendar__head">
      <span class="kosmos-calendar__month">{{ monthYearLabel }}</span>
      <div class="kosmos-calendar__nav">
        <button
          type="button"
          class="kosmos-calendar__navbtn"
          aria-label="Предыдущая неделя"
          @click="shiftWeek(-1)"
        >
          <ChevronLeft :size="14" :stroke-width="1.8" />
        </button>
        <button
          type="button"
          class="kosmos-calendar__navbtn"
          aria-label="Следующая неделя"
          @click="shiftWeek(1)"
        >
          <ChevronRight :size="14" :stroke-width="1.8" />
        </button>
      </div>
    </header>

    <div class="kosmos-calendar__strip">
      <button
        v-for="c in cells"
        :key="c.iso"
        type="button"
        class="kosmos-calendar__cell"
        :class="{
          'kosmos-calendar__cell--today': c.isToday,
          'kosmos-calendar__cell--selected': c.isSelected,
        }"
        @click="pickCell(c)"
      >
        <span class="kosmos-calendar__wkday">{{ c.weekday }}</span>
        <span class="kosmos-calendar__day">{{ c.date.getDate() }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.kosmos-calendar {
  display: flex;
  flex-direction: column;
  gap: 0.625rem;
  padding: 0.75rem;
  background: var(--popover, var(--background));
  color: var(--popover-foreground, var(--foreground));
  font-family: var(--font-sans, inherit);
}

.kosmos-calendar__head {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.kosmos-calendar__month {
  font-size: 0.875rem;
  font-weight: 600;
}

.kosmos-calendar__nav {
  display: flex;
  gap: 0.25rem;
}

.kosmos-calendar__navbtn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
  border-radius: 6px;
  cursor: pointer;
  transition: background-color 160ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-calendar__navbtn:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.kosmos-calendar__strip {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 4px;
}

.kosmos-calendar__cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  padding: 0.375rem 0;
  border: none;
  background: transparent;
  color: var(--foreground);
  border-radius: 8px;
  cursor: pointer;
  font-family: inherit;
  transition: background-color 180ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-calendar__cell:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.kosmos-calendar__wkday {
  font-size: 0.625rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  font-weight: 600;
}

.kosmos-calendar__day {
  font-size: 0.9375rem;
  font-weight: 500;
  font-variant-numeric: tabular-nums;
  font-family: var(--font-mono, ui-monospace, monospace);
}

.kosmos-calendar__cell--today {
  outline: 1px solid color-mix(in srgb, var(--accent) 55%, transparent);
}

.kosmos-calendar__cell--selected {
  background: var(--accent);
  color: var(--accent-foreground);
}

.kosmos-calendar__cell--selected .kosmos-calendar__wkday {
  color: color-mix(in srgb, var(--accent-foreground) 75%, transparent);
}

.kosmos-calendar__cell--selected:hover {
  background: var(--accent);
}
</style>
