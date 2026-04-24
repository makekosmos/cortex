import { ipcMain } from "electron";
import type { AppSettings } from "../services/contracts";
import type { WithRuntime } from "./types";

export interface AppIpcDeps {
  withRuntime: WithRuntime;
  getArkConnectionInfo: () => {
    ark_db_path: string;
    ark_db_exists: boolean;
    ark_db_directory: string;
    uses_shared_selection: boolean;
    space_code: string | null;
    space_id: string | null;
    selection_source: string | null;
    vault_path: string | null;
  };
}

export function registerAppIpcHandlers(deps: AppIpcDeps) {
  const { withRuntime } = deps;

  ipcMain.handle(
    "get_all_settings",
    withRuntime(async ({ services }) => await services.settings.getAllSettings()),
  );

  ipcMain.handle(
    "update_settings",
    withRuntime(async ({ services }, payload: { settings: AppSettings }) => {
      await services.settings.updateSettings(payload.settings);
    }),
  );

  ipcMain.handle(
    "get_setting",
    withRuntime(
      async ({ services }, payload: { key: string }) =>
        await services.settings.getSetting(payload.key),
    ),
  );

  ipcMain.handle(
    "set_setting",
    withRuntime(async ({ services }, payload: { key: string; value: string }) => {
      await services.settings.setSetting(payload.key, payload.value);
    }),
  );

  ipcMain.handle(
    "get_ark_connection_info",
    withRuntime(async () => deps.getArkConnectionInfo()),
  );

  ipcMain.handle(
    "add_scan_directory",
    withRuntime(async ({ services }, payload: { path: string }) => {
      await services.settings.addScanDirectory(payload.path);
    }),
  );

  ipcMain.handle(
    "get_scan_directories",
    withRuntime(
      async ({ services }) => await services.settings.getScanDirectories(),
    ),
  );

  ipcMain.handle(
    "remove_scan_directory",
    withRuntime(async ({ services }, payload: { path: string }) => {
      await services.settings.removeScanDirectory(payload.path);
    }),
  );

  ipcMain.handle(
    "get_playtime_stats",
    withRuntime(
      async ({ services }, payload: { start?: string; end?: string } = {}) =>
        await services.stats.getPlaytimeStats(payload?.start, payload?.end),
    ),
  );

  ipcMain.handle(
    "get_system_info",
    withRuntime(async ({ services }) => await services.system.getSystemInfo()),
  );

  ipcMain.handle(
    "test_disk_speed",
    withRuntime(
      async ({ services }, payload: { mountPoint: string }) =>
        await services.system.testDiskSpeed(payload.mountPoint),
    ),
  );

  ipcMain.handle(
    "list_notifications",
    withRuntime(
      async ({ services }, payload: { unread_only: boolean }) =>
        await services.notifications.listNotifications(payload.unread_only),
    ),
  );

  ipcMain.handle(
    "create_notification",
    withRuntime(
      async (
        { services },
        payload: {
          level: string;
          title: string;
          message: string;
          source?: string;
        },
      ) =>
        await services.notifications.createNotification(
          payload.level,
          payload.title,
          payload.message,
          payload.source,
        ),
    ),
  );

  ipcMain.handle(
    "mark_notification_read",
    withRuntime(
      async ({ services }, payload: { id: string }) =>
        await services.notifications.markNotificationRead(payload.id),
    ),
  );

  ipcMain.handle(
    "mark_all_notifications_read",
    withRuntime(
      async ({ services }) => await services.notifications.markAllNotificationsRead(),
    ),
  );

  ipcMain.handle(
    "clear_notifications",
    withRuntime(
      async ({ services }) => await services.notifications.clearNotifications(),
    ),
  );

  ipcMain.handle(
    "get_catalogue_items",
    withRuntime(
      async ({ services }, payload: { source: string | null }) =>
        await services.catalogue.getCatalogueItems(payload.source),
    ),
  );

  ipcMain.handle(
    "search_catalogue",
    withRuntime(
      async ({ services }, payload: { query: string; source: string | null }) =>
        await services.catalogue.searchCatalogue(payload.query, payload.source),
    ),
  );

  ipcMain.handle(
    "upsert_catalogue_item",
    withRuntime(
      async (
        { services },
        payload: {
          input: {
            id?: number;
            rawg_id: number;
            name: string;
            payload: string;
            source?: string;
          };
        },
      ) =>
        await services.catalogue.upsertCatalogueItem({
          id: payload.input.id,
          rawgId: payload.input.rawg_id,
          name: payload.input.name,
          payload: payload.input.payload,
          source: payload.input.source,
        }),
    ),
  );

  ipcMain.handle(
    "delete_catalogue_item",
    withRuntime(
      async ({ services }, payload: { id: number }) =>
        await services.catalogue.deleteCatalogueItem(payload.id),
    ),
  );

  ipcMain.handle(
    "sync_library_to_catalogue",
    withRuntime(
      async ({ services }) => await services.catalogue.syncLibraryToCatalogue(),
    ),
  );
}
