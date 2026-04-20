import type {
  CalendarSurfaceEntry,
  CalendarViewMode,
} from "@/services/calendar/contracts";

export const DAY_MS = 24 * 60 * 60 * 1000;

const weekdayLongFormatter = new Intl.DateTimeFormat("ru-RU", {
  weekday: "long",
});
const weekdayShortFormatter = new Intl.DateTimeFormat("ru-RU", {
  weekday: "short",
});
const monthDayFormatter = new Intl.DateTimeFormat("ru-RU", {
  day: "numeric",
  month: "short",
});
const monthFormatter = new Intl.DateTimeFormat("ru-RU", {
  month: "long",
  year: "numeric",
});
const dayFormatter = new Intl.DateTimeFormat("ru-RU", {
  day: "numeric",
  month: "long",
  year: "numeric",
});
const timeFormatter = new Intl.DateTimeFormat("ru-RU", {
  hour: "2-digit",
  minute: "2-digit",
});

export function startOfDay(date: Date): Date {
  const next = new Date(date);
  next.setHours(0, 0, 0, 0);
  return next;
}

export function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

export function startOfWeek(date: Date): Date {
  const next = startOfDay(date);
  const day = next.getDay();
  const mondayOffset = day === 0 ? -6 : 1 - day;
  next.setDate(next.getDate() + mondayOffset);
  return next;
}

export function startOfMonth(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), 1);
}

export function toDayKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function fromDayKey(dayKey: string): Date {
  const [year, month, day] = dayKey.split("-").map(Number);
  return new Date(year, month - 1, day);
}

export function buildVisibleDays(
  viewMode: Exclude<CalendarViewMode, "month">,
  anchorDate: Date,
): Date[] {
  if (viewMode === "day") {
    return [startOfDay(anchorDate)];
  }

  if (viewMode === "four-days") {
    const start = startOfDay(anchorDate);
    return Array.from({ length: 4 }, (_, index) => addDays(start, index));
  }

  const start = startOfWeek(anchorDate);
  return Array.from({ length: 7 }, (_, index) => addDays(start, index));
}

export function buildMonthGridDays(anchorDate: Date): Date[] {
  const monthStart = startOfMonth(anchorDate);
  const gridStart = startOfWeek(monthStart);
  return Array.from({ length: 42 }, (_, index) => addDays(gridStart, index));
}

export function shiftAnchorDate(
  anchorDate: Date,
  viewMode: CalendarViewMode,
  direction: -1 | 1,
): Date {
  if (viewMode === "day") return addDays(anchorDate, direction);
  if (viewMode === "four-days") return addDays(anchorDate, direction * 4);
  if (viewMode === "week") return addDays(anchorDate, direction * 7);
  return new Date(anchorDate.getFullYear(), anchorDate.getMonth() + direction, 1);
}

export function formatRangeLabel(viewMode: CalendarViewMode, anchorDate: Date): string {
  if (viewMode === "month") {
    return capitalize(monthFormatter.format(anchorDate));
  }

  if (viewMode === "day") {
    return capitalize(dayFormatter.format(anchorDate));
  }

  const days = buildVisibleDays(viewMode, anchorDate);
  const first = days[0];
  const last = days[days.length - 1];

  if (first.getMonth() === last.getMonth() && first.getFullYear() === last.getFullYear()) {
    return `${monthDayFormatter.format(first)} - ${last.getDate()} ${capitalize(monthFormatter.format(last).split(" ")[0] ?? "")}`;
  }

  return `${monthDayFormatter.format(first)} - ${monthDayFormatter.format(last)}`;
}

export function weekdayShortLabel(date: Date): string {
  return capitalize(weekdayShortFormatter.format(date));
}

export function weekdayLongLabel(date: Date): string {
  return capitalize(weekdayLongFormatter.format(date));
}

export function isSameDay(left: Date, right: Date): boolean {
  return toDayKey(left) === toDayKey(right);
}

export function parseEntryBoundary(value: string, isAllDay: boolean): Date {
  if (!isAllDay) {
    return new Date(value);
  }
  return fromDayKey(value);
}

export function entryIntersectsDay(entry: CalendarSurfaceEntry, day: Date): boolean {
  const dayStart = startOfDay(day);
  const dayEnd = addDays(dayStart, 1);
  const entryStart = parseEntryBoundary(entry.start, entry.isAllDay);
  const entryEnd = parseEntryBoundary(entry.end, entry.isAllDay);
  return entryStart < dayEnd && entryEnd > dayStart;
}

export function getEntryDaySpan(entry: CalendarSurfaceEntry, day: Date) {
  const dayStart = startOfDay(day);
  const dayEnd = addDays(dayStart, 1);
  const entryStart = parseEntryBoundary(entry.start, entry.isAllDay);
  const entryEnd = parseEntryBoundary(entry.end, entry.isAllDay);
  return {
    start: new Date(Math.max(entryStart.getTime(), dayStart.getTime())),
    end: new Date(Math.min(entryEnd.getTime(), dayEnd.getTime())),
  };
}

export function formatEntryTime(entry: CalendarSurfaceEntry, day?: Date): string {
  if (entry.isAllDay) return "Весь день";

  const start = new Date(entry.start);
  const end = new Date(entry.end);

  if (!day) {
    return `${timeFormatter.format(start)} - ${timeFormatter.format(end)}`;
  }

  const span = getEntryDaySpan(entry, day);
  return `${timeFormatter.format(span.start)} - ${timeFormatter.format(span.end)}`;
}

export function capitalize(value: string): string {
  if (!value) return value;
  return value.charAt(0).toUpperCase() + value.slice(1);
}
