// Типы Dashboard view'ев. Концепция spaces убрана 2026-05-15 — одна БД на
// юзера, Dashboard сразу открывается на единственный список объектов.

export interface DashboardObjectType {
  id: string;
  name: string;
}

export interface DashboardObjectRow {
  id: string;
  typeId: string;
  primary: string;
  createdAt: string;
}

export interface DashboardUsageRow {
  id: string;
  processName: string;
  displayName: string;
  totalMs: number;
  idleMs: number;
  sessions: number;
  lastSeenAt?: string | null;
  normalizedPath: string;
}
