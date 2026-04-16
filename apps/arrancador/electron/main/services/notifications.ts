import { execute, queryAll } from "../helpers/db";
import { createNotificationId, mapNotificationRow, type NotificationRow } from "../helpers/notifications";
import type { DbLike, NotificationItem } from "../helpers/shared";

export interface NotificationsServiceDeps {
  db: DbLike;
}

export interface NotificationsService {
  listNotifications(unreadOnly?: boolean): Promise<NotificationItem[]>;
  createNotification(
    level: string,
    title: string,
    message: string,
    source?: string | null,
  ): Promise<NotificationItem>;
  markNotificationRead(id: string): Promise<boolean>;
  markAllNotificationsRead(): Promise<number>;
  clearNotifications(): Promise<number>;
  notifySuccess(title: string, message: string): Promise<NotificationItem>;
  notifyInfo(title: string, message: string): Promise<NotificationItem>;
  notifyWarning(title: string, message: string): Promise<NotificationItem>;
  notifyError(title: string, message: string): Promise<NotificationItem>;
}

async function insertNotification(
  db: DbLike,
  level: string,
  title: string,
  message: string,
  source?: string | null,
): Promise<NotificationItem> {
  const id = createNotificationId();
  const createdAt = new Date().toISOString();
  const normalizedSource = source?.trim() ? source.trim() : null;

  await execute(
    db,
    `INSERT INTO notifications (id, level, title, message, source, created_at, read_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)`,
    [id, level, title, message, normalizedSource, createdAt],
  );

  return {
    id,
    level,
    title,
    message,
    source: normalizedSource,
    created_at: createdAt,
    read_at: null,
  };
}

export function createNotificationsService(
  deps: NotificationsServiceDeps,
): NotificationsService {
  return {
    async listNotifications(unreadOnly = false): Promise<NotificationItem[]> {
      const rows = unreadOnly
        ? await queryAll<NotificationRow>(
            deps.db,
            `SELECT id, level, title, message, source, created_at, read_at
             FROM notifications
             WHERE read_at IS NULL
             ORDER BY created_at DESC
             LIMIT 200`,
          )
        : await queryAll<NotificationRow>(
            deps.db,
            `SELECT id, level, title, message, source, created_at, read_at
             FROM notifications
             ORDER BY created_at DESC
             LIMIT 200`,
          );

      return rows.map(mapNotificationRow);
    },

    async createNotification(
      level: string,
      title: string,
      message: string,
      source?: string | null,
    ): Promise<NotificationItem> {
      return await insertNotification(deps.db, level, title, message, source);
    },

    async markNotificationRead(id: string): Promise<boolean> {
      const updated = await execute(
        deps.db,
        "UPDATE notifications SET read_at = ?1 WHERE id = ?2 AND read_at IS NULL",
        [new Date().toISOString(), id],
      );
      return updated > 0;
    },

    async markAllNotificationsRead(): Promise<number> {
      return await execute(
        deps.db,
        "UPDATE notifications SET read_at = ?1 WHERE read_at IS NULL",
        [new Date().toISOString()],
      );
    },

    async clearNotifications(): Promise<number> {
      const rows = await queryAll<Pick<NotificationRow, "id">>(
        deps.db,
        "SELECT id FROM notifications",
      );
      await execute(deps.db, "DELETE FROM notifications");
      return rows.length;
    },

    async notifySuccess(title: string, message: string): Promise<NotificationItem> {
      return await insertNotification(deps.db, "success", title, message, null);
    },

    async notifyInfo(title: string, message: string): Promise<NotificationItem> {
      return await insertNotification(deps.db, "info", title, message, null);
    },

    async notifyWarning(title: string, message: string): Promise<NotificationItem> {
      return await insertNotification(deps.db, "warning", title, message, null);
    },

    async notifyError(title: string, message: string): Promise<NotificationItem> {
      return await insertNotification(deps.db, "error", title, message, null);
    },
  };
}
