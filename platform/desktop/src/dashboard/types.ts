// Типы Dashboard view'ев. Концепция spaces убрана 2026-05-15 — одна БД на
// юзера, Dashboard сразу открывается на единственный список объектов.

export interface DashboardObjectType {
  id: string;
  name: string;
}

export interface DashboardObjectRow {
  id: string;
  typeId: string;
  typeName: string;
  primary: string;
  createdAt: string;
  updatedAt: string;
}

export interface DashboardUsageRow {
  id: string;
  processName: string;
  displayName: string;
  iconRef?: string | null;
  runtimeMs: number;
  foregroundMs: number;
  idleMs: number;
  sessions: number;
  lastSeenAt?: string | null;
  normalizedPath: string;
}
