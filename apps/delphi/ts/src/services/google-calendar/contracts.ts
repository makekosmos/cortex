export type GoogleCalendarViewMode = "day" | "four-days" | "week" | "month";

export type GoogleCalendarRange = {
  start: string;
  end: string;
};

export type GoogleCalendarConfigSource = "none" | "saved" | "env";

export type GoogleCalendarConfigState = {
  clientId: string;
  hasClientSecret: boolean;
  source: GoogleCalendarConfigSource;
};

export type GoogleCalendarAccount = {
  email: string;
  name: string | null;
  pictureUrl: string | null;
};

export type GoogleCalendarStatus = {
  available: boolean;
  configured: boolean;
  connected: boolean;
  syncing: boolean;
  accountEmail: string | null;
  lastSyncAt: string | null;
  lastSyncError: string | null;
  coverageStart: string | null;
  coverageEnd: string | null;
};

export type GoogleCalendarCalendar = {
  id: string;
  summary: string;
  primary: boolean;
  selected: boolean;
  accessRole: string | null;
  backgroundColor: string | null;
  foregroundColor: string | null;
};

export type GoogleCalendarEvent = {
  id: string;
  calendarId: string;
  calendarSummary: string;
  title: string;
  description: string | null;
  location: string | null;
  status: string;
  htmlLink: string | null;
  start: string;
  end: string;
  isAllDay: boolean;
  color: string | null;
};

export type GoogleCalendarSnapshot = {
  config: GoogleCalendarConfigState;
  account: GoogleCalendarAccount | null;
  status: GoogleCalendarStatus;
  calendars: GoogleCalendarCalendar[];
  events: GoogleCalendarEvent[];
};

export type CalendarSurfaceEntry = {
  id: string;
  source: "task" | "google";
  title: string;
  subtitle: string | null;
  link: string | null;
  color: string | null;
  start: string;
  end: string;
  isAllDay: boolean;
};

export const EMPTY_GOOGLE_CALENDAR_SNAPSHOT: GoogleCalendarSnapshot = {
  config: {
    clientId: "",
    hasClientSecret: false,
    source: "none",
  },
  account: null,
  status: {
    available: false,
    configured: false,
    connected: false,
    syncing: false,
    accountEmail: null,
    lastSyncAt: null,
    lastSyncError: null,
    coverageStart: null,
    coverageEnd: null,
  },
  calendars: [],
  events: [],
};
