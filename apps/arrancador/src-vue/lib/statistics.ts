import type { GamePlaytime } from "@/types";

export type StatisticsRangePreset = "7d" | "30d" | "90d" | "month" | "custom";

export interface StatisticsRangePresetOption {
  id: Exclude<StatisticsRangePreset, "month" | "custom">;
  label: string;
  days: number;
}

export interface DailyTrendPoint {
  date: string;
  hours: number;
  seconds: number;
}

export interface PerGamePoint extends GamePlaytime {
  hours: number;
}

export const rangePresets: StatisticsRangePresetOption[] = [
  { id: "7d", label: "7 дней", days: 7 },
  { id: "30d", label: "30 дней", days: 30 },
  { id: "90d", label: "90 дней", days: 90 },
];

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

export const formatDuration = (seconds: number) => {
  const totalMinutes = Math.round(seconds / 60);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

  if (hours <= 0) {
    return `${minutes} мин`;
  }

  return `${hours} ч ${minutes} мин`;
};

export const formatDateShort = (value: string) => {
  const date = new Date(`${value}T00:00:00`);
  return date.toLocaleDateString("ru-RU", {
    day: "2-digit",
    month: "short",
  });
};

export const formatDateLong = (value: string) => {
  const date = new Date(`${value}T00:00:00`);
  return date.toLocaleDateString("ru-RU", {
    day: "2-digit",
    month: "long",
  });
};

export const formatDateMonthLabel = (value: string) => {
  const date = new Date(`${value}-01T00:00:00`);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleDateString("ru-RU", {
    year: "numeric",
    month: "long",
  });
};

export const formatMonthValue = (year: number, month: number) =>
  `${year}-${String(month).padStart(2, "0")}`;

export const getMonthRange = (value: string) => {
  const [yearValue, monthValue] = value.split("-");
  const year = Number(yearValue);
  const month = Number(monthValue);

  if (!year || !month) {
    return null;
  }

  const start = toIsoDate(new Date(year, month - 1, 1));
  const end = toIsoDate(new Date(year, month, 0));

  return { start, end };
};

export const getMonthValueFromRange = (start: string, end: string) => {
  const startParts = start.split("-").map(Number);
  const endParts = end.split("-").map(Number);

  if (startParts.length !== 3 || endParts.length !== 3) {
    return "";
  }

  const [startYear, startMonth, startDay] = startParts;
  const [endYear, endMonth, endDay] = endParts;

  if (
    !startYear ||
    !startMonth ||
    !startDay ||
    !endYear ||
    !endMonth ||
    !endDay
  ) {
    return "";
  }

  if (startYear !== endYear || startMonth !== endMonth || startDay !== 1) {
    return "";
  }

  const lastDay = new Date(startYear, startMonth, 0).getDate();
  if (endDay !== lastDay) {
    return "";
  }

  return formatMonthValue(startYear, startMonth);
};

export const buildRecentDateOptions = (today: Date, days: number) => {
  const options: string[] = [];

  for (let offset = 0; offset < days; offset += 1) {
    options.push(toIsoDate(addDays(today, -offset)));
  }

  return options;
};

export const buildMonthOptions = (today: Date, months: number) => {
  const options: string[] = [];

  for (let offset = 0; offset < months; offset += 1) {
    const date = new Date(today.getFullYear(), today.getMonth() - offset, 1);
    options.push(formatMonthValue(date.getFullYear(), date.getMonth() + 1));
  }

  return options;
};

export const formatGameName = (name: string) =>
  name.length > 28 ? `${name.slice(0, 25)}…` : name;
