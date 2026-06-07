import type { ResolvedNoteTypeField } from "./typedNotes";

const PLAY_STATUS_LABELS: Record<string, string> = {
  not_started: "\u041d\u0435 \u043d\u0430\u0447\u0430\u0442\u0430",
  in_progress: "\u0412 \u043f\u0440\u043e\u0446\u0435\u0441\u0441\u0435",
  completed: "\u041f\u0440\u043e\u0439\u0434\u0435\u043d\u0430",
  abandoned: "\u0417\u0430\u0431\u0440\u043e\u0448\u0435\u043d\u0430",
};

function readFiniteNumber(value: unknown): number | null {
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : null;
  }

  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }

  return null;
}

function hasMeaningfulScalarValue(value: unknown): boolean {
  if (value === null || value === undefined) {
    return false;
  }

  if (typeof value === "string") {
    return value.trim().length > 0;
  }

  return true;
}

export function formatReadableRussianDate(value: unknown): string {
  if (!hasMeaningfulScalarValue(value)) {
    return "";
  }

  const source =
    typeof value === "number" ? value : typeof value === "string" ? Date.parse(value) : Number.NaN;

  if (!Number.isFinite(source)) {
    return String(value);
  }

  const date = new Date(source);
  const day = new Intl.DateTimeFormat("ru-RU", { day: "numeric" }).format(date);
  const month = new Intl.DateTimeFormat("ru-RU", { month: "long" }).format(date);
  const year = new Intl.DateTimeFormat("ru-RU", { year: "numeric" }).format(date);
  return `${day} ${month} ${year} \u0433\u043e\u0434\u0430`;
}

export function formatPlaytimeSeconds(value: unknown): string {
  const totalSeconds = readFiniteNumber(value);
  if (totalSeconds === null) {
    return "";
  }

  const roundedSeconds = Math.max(0, Math.floor(totalSeconds));
  if (roundedSeconds < 60) {
    return "\u041c\u0435\u043d\u044c\u0448\u0435 \u043c\u0438\u043d\u0443\u0442\u044b";
  }

  const totalMinutes = Math.floor(roundedSeconds / 60);
  const days = Math.floor(totalMinutes / (60 * 24));
  const hours = Math.floor((totalMinutes % (60 * 24)) / 60);
  const minutes = totalMinutes % 60;

  if (days > 0) {
    return hours > 0 ? `${days} \u0434 ${hours} \u0447` : `${days} \u0434`;
  }

  if (hours > 0) {
    return minutes > 0 ? `${hours} \u0447 ${minutes} \u043c\u0438\u043d` : `${hours} \u0447`;
  }

  return `${minutes} \u043c\u0438\u043d`;
}

export function formatObjectFieldValue(field: ResolvedNoteTypeField, value: unknown): string {
  if (Array.isArray(value)) {
    return value.length ? value.map((item) => String(item)).join(", ") : "";
  }

  if (!hasMeaningfulScalarValue(value)) {
    return "";
  }

  if (field.kind === "boolean") {
    return value === true ? "\u0414\u0430" : "\u041d\u0435\u0442";
  }

  if (field.id === "play_status" && typeof value === "string") {
    return PLAY_STATUS_LABELS[value] ?? value;
  }

  if (field.id === "total_playtime_seconds") {
    return formatPlaytimeSeconds(value);
  }

  if (field.id === "last_played_at" || field.kind === "date") {
    return formatReadableRussianDate(value);
  }

  return String(value);
}
