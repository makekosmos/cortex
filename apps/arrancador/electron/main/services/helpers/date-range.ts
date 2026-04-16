const UTC_DAY_MS = 24 * 60 * 60 * 1000;

export function parseIsoDateOnly(value?: string | null): Date | null {
  if (!value) {
    return null;
  }

  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value.trim());
  if (!match) {
    return null;
  }

  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const date = new Date(Date.UTC(year, month - 1, day));

  if (
    date.getUTCFullYear() !== year ||
    date.getUTCMonth() !== month - 1 ||
    date.getUTCDate() !== day
  ) {
    return null;
  }

  return date;
}

export function formatIsoDateOnly(value: Date): string {
  return [
    value.getUTCFullYear(),
    String(value.getUTCMonth() + 1).padStart(2, "0"),
    String(value.getUTCDate()).padStart(2, "0"),
  ].join("-");
}

export function addUtcDays(value: Date, amount: number): Date {
  return new Date(value.getTime() + amount * UTC_DAY_MS);
}

export function getUtcToday() {
  const now = new Date();
  return new Date(
    Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()),
  );
}

export function normalizeDateRange(
  start?: string | null,
  end?: string | null,
  windowDays = 30,
) {
  const today = getUtcToday();
  let endDate = parseIsoDateOnly(end) ?? today;
  let startDate =
    parseIsoDateOnly(start) ?? addUtcDays(endDate, -(windowDays - 1));

  if (startDate > endDate) {
    [startDate, endDate] = [endDate, startDate];
  }

  return {
    startDate,
    endDate,
    rangeStart: formatIsoDateOnly(startDate),
    rangeEnd: formatIsoDateOnly(endDate),
  };
}

export function enumerateUtcDates(startDate: Date, endDate: Date): Date[] {
  const dates: Date[] = [];
  let cursor = new Date(startDate);

  while (cursor <= endDate) {
    dates.push(new Date(cursor));
    cursor = addUtcDays(cursor, 1);
  }

  return dates;
}
