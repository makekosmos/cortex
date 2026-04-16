import { randomUUID } from "node:crypto";
import type { NotificationItem } from "./shared";

export interface NotificationRow {
  id: string;
  level: string;
  title: string;
  message: string;
  source: string | null;
  created_at: string;
  read_at: string | null;
}

export function createNotificationId(): string {
  return randomUUID();
}

export function mapNotificationRow(row: NotificationRow): NotificationItem {
  return {
    id: row.id,
    level: row.level,
    title: row.title,
    message: row.message,
    source: row.source,
    created_at: row.created_at,
    read_at: row.read_at,
  };
}

