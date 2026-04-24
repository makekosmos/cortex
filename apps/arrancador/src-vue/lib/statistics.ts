import type { DailyPlaytime, GamePlaytime } from "@/types";

export interface DayStatsRow extends GamePlaytime {
  hours: number;
}

export interface HeatmapCell {
  date: string;
  seconds: number;
  hours: number;
  level: 0 | 1 | 2 | 3 | 4;
  inRange: boolean;
  isToday: boolean;
  isSelected: boolean;
}

export interface HeatmapWeek {
  key: string;
  monthLabel: string | null;
  cells: HeatmapCell[];
}

export const HEATMAP_RANGE_DAYS = 365;

export const toHours = (seconds: number) => Math.round((seconds / 3600) * 10) / 10;

export const toIsoDate = (value: Date) => {
  const year = value.getFullYear();
  const month = String(value.getMonth() + 1).padStart(2, "0");
  const day = String(value.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

export const addDays = (value: Date, amount: number) => {
  const nextDate = new Date(value);
  nextDate.setDate(nextDate.getDate() + amount);
  return nextDate;
};

export const startOfWeek = (value: Date) => {
  const nextDate = new Date(value);
  const weekDay = (nextDate.getDay() + 6) % 7;
  nextDate.setDate(nextDate.getDate() - weekDay);
  return nextDate;
};

export const endOfWeek = (value: Date) => addDays(startOfWeek(value), 6);

export const formatDuration = (seconds: number) => {
  const totalMinutes = Math.round(seconds / 60);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

  if (hours <= 0) {
    return `${minutes} мин`;
  }

  return `${hours} ч ${minutes} мин`;
};

export const formatHours = (seconds: number) => `${toHours(seconds).toFixed(1)} ч`;

export const formatDateLong = (value: string) => {
  const date = new Date(`${value}T00:00:00`);
  return date.toLocaleDateString("ru-RU", {
    day: "2-digit",
    month: "long",
  });
};

export const formatDateWithWeekday = (value: string) => {
  const date = new Date(`${value}T00:00:00`);
  return date.toLocaleDateString("ru-RU", {
    weekday: "long",
    day: "numeric",
    month: "long",
  });
};

export const formatMonthShort = (value: string) =>
  new Date(`${value}T00:00:00`).toLocaleDateString("ru-RU", {
    month: "short",
  });

export const formatGameName = (name: string) =>
  name.length > 28 ? `${name.slice(0, 25)}…` : name;

function resolveHeatLevel(seconds: number, maxSeconds: number): 0 | 1 | 2 | 3 | 4 {
  if (seconds <= 0 || maxSeconds <= 0) {
    return 0;
  }

  const ratio = seconds / maxSeconds;
  if (ratio >= 0.75) {
    return 4;
  }
  if (ratio >= 0.5) {
    return 3;
  }
  if (ratio >= 0.25) {
    return 2;
  }
  return 1;
}

export function buildHeatmapWeeks(
  dailyTotals: DailyPlaytime[],
  rangeStart: string,
  rangeEnd: string,
  selectedDate: string,
  todayDate: string,
): HeatmapWeek[] {
  const entriesByDate = new Map(dailyTotals.map((entry) => [entry.date, entry.seconds]));
  const maxSeconds = Math.max(...dailyTotals.map((entry) => entry.seconds), 0);
  const gridStart = startOfWeek(new Date(`${rangeStart}T00:00:00`));
  const gridEnd = endOfWeek(new Date(`${rangeEnd}T00:00:00`));
  const weeks: HeatmapWeek[] = [];

  let currentDate = new Date(gridStart);
  let weekIndex = 0;

  while (currentDate <= gridEnd) {
    const cells: HeatmapCell[] = [];
    let monthLabel: string | null = null;

    for (let weekdayIndex = 0; weekdayIndex < 7; weekdayIndex += 1) {
      const date = toIsoDate(currentDate);
      const inRange = date >= rangeStart && date <= rangeEnd;
      const seconds = inRange ? entriesByDate.get(date) ?? 0 : 0;

      if (!monthLabel && date.slice(8, 10) === "01" && inRange) {
        monthLabel = formatMonthShort(date);
      }

      cells.push({
        date,
        seconds,
        hours: toHours(seconds),
        level: resolveHeatLevel(seconds, maxSeconds),
        inRange,
        isToday: date === todayDate,
        isSelected: date === selectedDate,
      });
      currentDate = addDays(currentDate, 1);
    }

    weeks.push({
      key: `week-${weekIndex}`,
      monthLabel,
      cells,
    });
    weekIndex += 1;
  }

  return weeks;
}
