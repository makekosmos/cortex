function hoursFormatter(maximumFractionDigits: number): Intl.NumberFormat {
  return new Intl.NumberFormat("ru-RU", {
    minimumFractionDigits: maximumFractionDigits === 0 ? 0 : 1,
    maximumFractionDigits,
  });
}

export function formatDuration(ms: number): string {
  const totalMinutes = Math.round(ms / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

  if (hours === 0) {
    return `${minutes} мин`;
  }

  if (minutes === 0) {
    return `${hours} ч`;
  }

  return `${hours} ч ${minutes} мин`;
}

export function formatCompactHours(ms: number): string {
  const hours = ms / 3_600_000;
  const formatter = hoursFormatter(ms >= 36_000_000 ? 0 : 1);
  return `${formatter.format(hours)} ч`;
}

export function formatSessionCount(count: number): string {
  const lastTwoDigits = count % 100;
  const lastDigit = count % 10;

  if (lastTwoDigits >= 11 && lastTwoDigits <= 14) {
    return `${count} сессий`;
  }

  if (lastDigit === 1) {
    return `${count} сессия`;
  }

  if (lastDigit >= 2 && lastDigit <= 4) {
    return `${count} сессии`;
  }

  return `${count} сессий`;
}

export function formatDateLabel(date: string): string {
  const value = new Date(`${date}T00:00:00.000Z`);
  return new Intl.DateTimeFormat("ru-RU", {
    day: "2-digit",
    month: "short",
  }).format(value);
}

export function formatDateTimeLabel(dateTime: string | null): string {
  if (!dateTime) {
    return "—";
  }

  return new Intl.DateTimeFormat("ru-RU", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(dateTime));
}

export function formatRelativeRange(startedAt: string, endedAt: string | null): string {
  const endLabel = endedAt ? formatDateTimeLabel(endedAt) : "сейчас";
  return `${formatDateTimeLabel(startedAt)} → ${endLabel}`;
}

export function weekdayLabel(weekday: number): string {
  return ["Вс", "Пн", "Вт", "Ср", "Чт", "Пт", "Сб"][weekday] ?? "?";
}
