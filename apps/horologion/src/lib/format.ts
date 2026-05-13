export function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

const RU_WEEKDAYS = ["Вс", "Пн", "Вт", "Ср", "Чт", "Пт", "Сб"] as const;
const RU_MONTHS_GEN = [
  "января",
  "февраля",
  "марта",
  "апреля",
  "мая",
  "июня",
  "июля",
  "августа",
  "сентября",
  "октября",
  "ноября",
  "декабря",
] as const;

export function formatDayHeader(iso: string): string {
  const d = new Date(iso);
  const weekday = RU_WEEKDAYS[d.getDay()];
  const day = d.getDate();
  const month = RU_MONTHS_GEN[d.getMonth()];
  return `${weekday}, ${day} ${month}`;
}

export function dayKey(iso: string): string {
  return iso.slice(0, 10); // YYYY-MM-DD
}
