// Типы Dashboard view'ев. SpaceMeta совпадает с тем, что отдаёт IPC
// `kepler:spaces:list` (см. shell/electron/main.ts).

export interface SpaceMeta {
  id: string;
  name: string;
  objectCount: number | null;
  lastAccessedAt: number;
  label: string;
  isSelected: boolean;
}

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
