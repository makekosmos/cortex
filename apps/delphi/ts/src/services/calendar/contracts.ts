export type CalendarViewMode = "day" | "four-days" | "week" | "month";

export type CalendarSurfaceEntry = {
  id: string;
  title: string;
  subtitle: string | null;
  link: string | null;
  color: string | null;
  start: string;
  end: string;
  isAllDay: boolean;
};
