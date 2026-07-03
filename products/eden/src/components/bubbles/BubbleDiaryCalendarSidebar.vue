<script setup lang="ts">
import { computed } from "vue";
import { bubbleDateKey, formatBubbleDateKey, type BubbleTimelineNode } from "./bubbleDiaryModel";

type CalendarDay = {
  key: string;
  dayName: string;
  dayNumber: number;
  count: number;
  today: boolean;
  weekEdge: boolean;
};

type CalendarWeek = {
  key: string;
  label: string;
  days: CalendarDay[];
};

const props = defineProps<{
  bubbles: BubbleTimelineNode[];
}>();

const emit = defineEmits<{
  "date-select": [date: string];
}>();

const DAY_NAMES = ["S", "M", "T", "W", "T", "F", "S"];

const todayKey = computed(() => formatBubbleDateKey(new Date()));

const dateCounts = computed(() => {
  const counts = new Map<string, number>();
  for (const bubble of props.bubbles) {
    const key = bubbleDateKey(bubble);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return counts;
});

const oldestDate = computed(() => {
  let oldest = startOfDay(new Date());
  for (const key of dateCounts.value.keys()) {
    const date = parseDateKey(key);
    if (date && date < oldest) oldest = date;
  }
  return oldest;
});

const weeks = computed<CalendarWeek[]>(() =>
  buildWeeks(oldestDate.value, dateCounts.value, todayKey.value),
);

function buildWeeks(oldest: Date, counts: Map<string, number>, today: string): CalendarWeek[] {
  const weeksList: CalendarWeek[] = [];
  const current = startOfDay(new Date());
  const firstWeekEnd = new Date(current);

  while (current.getDay() !== 1) current.setDate(current.getDate() - 1);

  let weekStart = new Date(current);
  let weekEnd = firstWeekEnd;
  weeksList.push(createWeek(weekStart, weekEnd, counts, today));

  const paddedOldest = new Date(oldest);
  paddedOldest.setDate(paddedOldest.getDate() - 40);

  while (weekStart > paddedOldest) {
    weekEnd = new Date(weekStart);
    weekEnd.setDate(weekEnd.getDate() - 1);
    weekStart = new Date(weekEnd);
    weekStart.setDate(weekStart.getDate() - 6);
    weeksList.push(createWeek(new Date(weekStart), new Date(weekEnd), counts, today));
  }

  return weeksList;
}

function createWeek(
  start: Date,
  end: Date,
  counts: Map<string, number>,
  today: string,
): CalendarWeek {
  const days: CalendarDay[] = [];
  const cursor = new Date(start);
  while (cursor <= end) {
    const day = new Date(cursor);
    const key = formatBubbleDateKey(day);
    days.push({
      key,
      dayName: DAY_NAMES[day.getDay()],
      dayNumber: day.getDate(),
      count: counts.get(key) ?? 0,
      today: key === today,
      weekEdge: day.getDay() === 0,
    });
    cursor.setDate(cursor.getDate() + 1);
  }

  return {
    key: `${formatBubbleDateKey(start)}:${formatBubbleDateKey(end)}`,
    label: `${monthLabel(start)} ${start.getFullYear()}`,
    days: days.reverse(),
  };
}

function parseDateKey(key: string): Date | null {
  const match = key.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!match) return null;
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
}

function startOfDay(date: Date): Date {
  const next = new Date(date);
  next.setHours(0, 0, 0, 0);
  return next;
}

function monthLabel(date: Date): string {
  return date.toLocaleDateString("ru-RU", { month: "short" }).replace(".", "");
}

function dotsForCount(count: number): number[] {
  return Array.from({ length: Math.min(count, 48) }, (_, index) => index);
}
</script>

<template>
  <aside
    class="bubble-calendar-sidebar"
    data-testid="diary-calendar-sidebar"
    aria-label="Календарь дневника"
  >
    <div class="bubble-calendar-sidebar__timeline kosmos-scroll">
      <section v-for="week in weeks" :key="week.key" class="bubble-calendar-sidebar__week">
        <div class="bubble-calendar-sidebar__week-label">{{ week.label }}</div>
        <button
          v-for="day in week.days"
          :key="day.key"
          class="bubble-calendar-sidebar__day"
          :class="{
            'bubble-calendar-sidebar__day--today': day.today,
            'bubble-calendar-sidebar__day--week-edge': day.weekEdge,
          }"
          type="button"
          :title="day.key"
          :data-testid="`diary-calendar-day-${day.key}`"
          :data-entry-count="day.count"
          @click="emit('date-select', day.key)"
        >
          <span class="bubble-calendar-sidebar__counts" aria-hidden="true">
            <span
              v-for="dot in dotsForCount(day.count)"
              :key="dot"
              class="bubble-calendar-sidebar__count"
            />
          </span>
          <span class="bubble-calendar-sidebar__day-line" aria-hidden="true" />
          <span class="bubble-calendar-sidebar__day-name">{{ day.dayName }}</span>
          <span class="bubble-calendar-sidebar__day-number">{{ day.dayNumber }}</span>
        </button>
        <span class="bubble-calendar-sidebar__week-line" aria-hidden="true" />
      </section>
      <span class="bubble-calendar-sidebar__scrubber" aria-hidden="true" />
    </div>
  </aside>
</template>

<style scoped>
.bubble-calendar-sidebar {
  --bubble-calendar-sidebar-width: 180px;
  width: var(--bubble-calendar-sidebar-width);
  min-width: 168px;
  max-width: 196px;
  height: 100%;
  box-sizing: border-box;
  flex: 0 0 var(--bubble-calendar-sidebar-width);
  background: var(--sidebar-bg);
  color: color-mix(in srgb, var(--muted-foreground) 84%, var(--foreground));
  -webkit-app-region: no-drag;
}

.bubble-calendar-sidebar::before {
  position: fixed;
  top: 0;
  bottom: 0;
  left: calc(100vw - var(--bubble-calendar-sidebar-width));
  z-index: 40;
  width: 1px;
  background: var(--border-color-strong);
  content: "";
  pointer-events: none;
}

.bubble-calendar-sidebar__timeline {
  position: relative;
  height: 100%;
  overflow-x: hidden;
  overflow-y: auto;
  padding: 10px 0 18px;
  font-size: 0.8rem;
  user-select: none;
  scrollbar-width: none;
}

.bubble-calendar-sidebar__timeline::-webkit-scrollbar {
  display: none;
}

.bubble-calendar-sidebar__week {
  position: relative;
  display: flex;
  flex-direction: column;
  padding: 0 10px 0 24px;
}

.bubble-calendar-sidebar__week-label {
  position: absolute;
  bottom: -6px;
  left: 28px;
  color: color-mix(in srgb, var(--muted-foreground) 56%, transparent);
  font-size: 0.72rem;
  font-weight: 520;
  line-height: 1;
  white-space: nowrap;
}

.bubble-calendar-sidebar__week-line {
  display: none;
  width: 60px;
  height: 1px;
  align-self: flex-end;
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
}

.bubble-calendar-sidebar__day {
  display: flex;
  height: 22px;
  width: 100%;
  align-items: center;
  justify-content: flex-end;
  border: 0;
  background: transparent;
  color: inherit;
  padding: 0;
  font: inherit;
  text-align: right;
  cursor: pointer;
  transition:
    color 120ms ease,
    opacity 120ms ease;
}

.bubble-calendar-sidebar__counts {
  display: flex;
  max-width: 82px;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: flex-end;
  margin-right: 5px;
}

.bubble-calendar-sidebar__count {
  width: 4px;
  height: 4px;
  margin: 1px 2px 1px 0;
  border-radius: var(--radius-pill, 999px);
  background: currentColor;
  opacity: 0.9;
}

.bubble-calendar-sidebar__day-line {
  width: 20px;
  height: 1px;
  margin-right: 8px;
  background: currentColor;
  opacity: 0.24;
  transition: width 120ms ease;
}

.bubble-calendar-sidebar__day-name {
  width: 10px;
  font-weight: 650;
}

.bubble-calendar-sidebar__day-number {
  width: 18px;
  margin-right: 4px;
  opacity: 0.62;
  transition: opacity 120ms ease;
}

.bubble-calendar-sidebar__day--today {
  color: var(--accent);
}

.bubble-calendar-sidebar__day--week-edge {
  color: color-mix(in srgb, var(--accent) 72%, var(--foreground));
}

.bubble-calendar-sidebar__day:hover .bubble-calendar-sidebar__day-line,
.bubble-calendar-sidebar__day:focus-visible .bubble-calendar-sidebar__day-line {
  width: 25px;
}

.bubble-calendar-sidebar__day:hover .bubble-calendar-sidebar__day-number,
.bubble-calendar-sidebar__day:focus-visible .bubble-calendar-sidebar__day-number {
  opacity: 1;
}

.bubble-calendar-sidebar__day:focus-visible {
  outline: 1px solid color-mix(in srgb, var(--accent) 64%, transparent);
  outline-offset: 2px;
}

.bubble-calendar-sidebar__scrubber {
  position: absolute;
  top: 10px;
  left: 10px;
  width: calc(100% - 85px);
  height: 1px;
  background: var(--accent);
  transition:
    top 500ms cubic-bezier(0.075, 0.82, 0.165, 1),
    height 120ms ease;
}

.bubble-calendar-sidebar__scrubber::before {
  position: absolute;
  top: -20px;
  left: 0;
  width: 100%;
  height: 40px;
  content: "";
}

.bubble-calendar-sidebar__scrubber:hover {
  height: 5px;
  cursor: grab;
}
</style>
