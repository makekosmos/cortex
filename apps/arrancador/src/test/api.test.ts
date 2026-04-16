import {
  achievementsApi,
  backupApi,
  catalogueApi,
  gamesApi,
  metadataApi,
  notificationsApi,
  scanApi,
  settingsApi,
  statsApi,
  systemApi,
} from "@/lib/api";
import type { AppSettings, NewGame, UpdateGame } from "@/types";
import { arrancadorInvokeMock } from "./bridge";

const baseSettings: AppSettings = {
  theme: "dark",
  ludusavi_path: "native",
  backup_directory: "C:\\Backups",
  auto_backup: true,
  backup_before_launch: true,
  backup_compression_enabled: true,
  backup_compression_level: 60,
  backup_skip_compression_once: false,
  max_backups_per_game: 5,
  rawg_api_key: "rawg-key",
  start_minimized_in_tray: false,
};

const assertInvokeCall = (command: string, args?: Record<string, unknown>) => {
  if (args === undefined) {
    expect(arrancadorInvokeMock).toHaveBeenLastCalledWith(command);
  } else {
    expect(arrancadorInvokeMock).toHaveBeenLastCalledWith(command, args);
  }
};

const runInvokeCase = async (
  operation: () => Promise<unknown>,
  command: string,
  args?: Record<string, unknown>,
) => {
  arrancadorInvokeMock.mockResolvedValueOnce(undefined);
  await operation();
  assertInvokeCall(command, args);
};

describe("gamesApi", () => {
  it("dispatches game commands", async () => {
    const newGame: NewGame = {
      name: "Arcadia",
      exe_path: "C:\\Games\\Arcadia\\arcadia.exe",
      exe_name: "arcadia.exe",
    };
    const updateGame: UpdateGame = {
      id: "game-1",
      name: "Arcadia Prime",
      is_favorite: true,
    };

    await runInvokeCase(() => gamesApi.getAll(), "get_all_games");
    await runInvokeCase(() => gamesApi.get("game-1"), "get_game", {
      id: "game-1",
    });
    await runInvokeCase(() => gamesApi.add(newGame), "add_game", {
      game: newGame,
    });
    await runInvokeCase(() => gamesApi.addBatch([newGame]), "add_games_batch", {
      games: [newGame],
    });
    await runInvokeCase(() => gamesApi.update(updateGame), "update_game", {
      update: updateGame,
    });
    await runInvokeCase(() => gamesApi.delete("game-1"), "delete_game", {
      id: "game-1",
    });
    await runInvokeCase(
      () => gamesApi.toggleFavorite("game-1"),
      "toggle_favorite",
      { id: "game-1" },
    );
    await runInvokeCase(() => gamesApi.getFavorites(), "get_favorites");
    await runInvokeCase(
      () => gamesApi.recordLaunch("game-1"),
      "record_game_launch",
      { id: "game-1" },
    );
    await runInvokeCase(() => gamesApi.search("arcadia"), "search_games", {
      query: "arcadia",
    });
    await runInvokeCase(
      () => gamesApi.existsByPath("C:\\Games\\Arcadia\\arcadia.exe"),
      "game_exists_by_path",
      { exePath: "C:\\Games\\Arcadia\\arcadia.exe" },
    );
    await runInvokeCase(
      () => gamesApi.isInstalled("game-1"),
      "is_game_installed",
      { id: "game-1" },
    );
    await runInvokeCase(() => gamesApi.launch("game-1"), "launch_game", {
      id: "game-1",
    });
    await runInvokeCase(
      () => gamesApi.getRunningInstances("game-1"),
      "get_running_instances",
      { id: "game-1" },
    );
    await runInvokeCase(
      () => gamesApi.killProcesses("game-1"),
      "kill_game_processes",
      { id: "game-1" },
    );
    await runInvokeCase(
      () => gamesApi.resolveShortcutTarget("C:\\Games\\Arcadia\\arcadia.lnk"),
      "resolve_shortcut_target",
      { path: "C:\\Games\\Arcadia\\arcadia.lnk" },
    );
  });
});

describe("metadataApi", () => {
  it("dispatches metadata commands", async () => {
    await runInvokeCase(() => metadataApi.search("arcadia"), "search_rawg", {
      query: "arcadia",
    });
    await runInvokeCase(
      () => metadataApi.getDetails(1101),
      "get_rawg_game_details",
      { rawgId: 1101 },
    );
    await runInvokeCase(
      () => metadataApi.apply("game-1", 1101, true),
      "apply_rawg_metadata",
      { gameId: "game-1", rawgId: 1101, rename: true },
    );
    await runInvokeCase(
      () => metadataApi.setApiKey("rawg-key"),
      "set_rawg_api_key",
      { key: "rawg-key" },
    );
    await runInvokeCase(() => metadataApi.getApiKey(), "get_rawg_api_key");
  });
});

describe("achievementsApi", () => {
  it("dispatches achievement commands", async () => {
    await runInvokeCase(() => achievementsApi.getAll(false), "get_all_achievements", {
      unlockedOnly: false,
    });
    await runInvokeCase(() => achievementsApi.getAll(true), "get_all_achievements", {
      unlockedOnly: true,
    });
    await runInvokeCase(() => achievementsApi.seedDefaults(), "seed_default_achievements");
    await runInvokeCase(
      () => achievementsApi.recordEvent("game_launch", "Arcadia"),
      "record_achievement_event",
      { eventTrigger: "game_launch", eventContext: "Arcadia" },
    );
  });
});

describe("backupApi", () => {
  it("dispatches backup commands", async () => {
    await runInvokeCase(
      () => backupApi.checkLudusaviInstalled(),
      "check_ludusavi_installed",
    );
    await runInvokeCase(
      () => backupApi.getLudusaviPath(),
      "get_ludusavi_executable_path",
    );
    await runInvokeCase(
      () => backupApi.setLudusaviPath("C:\\Tools\\ludusavi.exe"),
      "set_ludusavi_path",
      { path: "C:\\Tools\\ludusavi.exe" },
    );
    await runInvokeCase(
      () => backupApi.setBackupDirectory("C:\\Backups"),
      "set_backup_directory",
      { path: "C:\\Backups" },
    );
    await runInvokeCase(
      () => backupApi.getBackupDirectory(),
      "get_backup_directory_setting",
    );
    await runInvokeCase(
      () => backupApi.refreshSqobaManifest(),
      "refresh_sqoba_manifest",
    );
    await runInvokeCase(
      () => backupApi.findGameSavePaths("Arcadia", "game-1"),
      "find_game_save_paths",
      { gameName: "Arcadia", gameId: "game-1" },
    );
    await runInvokeCase(
      () => backupApi.findGameSaves("Arcadia", "game-1"),
      "find_game_saves",
      { gameName: "Arcadia", gameId: "game-1" },
    );
    await runInvokeCase(
      () => backupApi.create("game-1", "Arcadia", true, "notes"),
      "create_backup",
      { gameId: "game-1", gameName: "Arcadia", isAuto: true, notes: "notes" },
    );
    await runInvokeCase(
      () => backupApi.getForGame("game-1"),
      "get_game_backups",
      { gameId: "game-1" },
    );
    await runInvokeCase(() => backupApi.restore("backup-1"), "restore_backup", {
      backupId: "backup-1",
    });
    await runInvokeCase(() => backupApi.delete("backup-1"), "delete_backup", {
      backupId: "backup-1",
    });
    await runInvokeCase(
      () => backupApi.shouldBackupBeforeLaunch("game-1"),
      "should_backup_before_launch",
      { gameId: "game-1" },
    );
    await runInvokeCase(
      () => backupApi.checkBackupNeeded("game-1", "Arcadia"),
      "check_backup_needed",
      { gameId: "game-1", gameName: "Arcadia" },
    );
    await runInvokeCase(
      () => backupApi.checkRestoreNeeded("game-1", "Arcadia"),
      "check_restore_needed",
      { gameId: "game-1", gameName: "Arcadia" },
    );
    await runInvokeCase(() => backupApi.getSettings(), "get_backup_settings");
    await runInvokeCase(
      () => backupApi.updateSettings({ max_backups: "3" }),
      "update_backup_settings",
      { settings: { max_backups: "3" } },
    );
  });
});

describe("notificationsApi", () => {
  it("dispatches notification commands", async () => {
    await runInvokeCase(
      () => notificationsApi.list(),
      "list_notifications",
      { unread_only: false },
    );
    await runInvokeCase(
      () => notificationsApi.list(true),
      "list_notifications",
      { unread_only: true },
    );
    await runInvokeCase(
      () => notificationsApi.create("info", "Title", "Body", "toast"),
      "create_notification",
      {
        level: "info",
        title: "Title",
        message: "Body",
        source: "toast",
      },
    );
    await runInvokeCase(
      () => notificationsApi.markRead("note-1"),
      "mark_notification_read",
      { id: "note-1" },
    );
    await runInvokeCase(
      () => notificationsApi.markAllRead(),
      "mark_all_notifications_read",
    );
    await runInvokeCase(() => notificationsApi.clear(), "clear_notifications");
  });
});

describe("catalogueApi", () => {
  it("dispatches catalogue commands", async () => {
    await runInvokeCase(
      () => catalogueApi.getItems(),
      "get_catalogue_items",
      { source: null },
    );
    await runInvokeCase(
      () => catalogueApi.getItems("rawg"),
      "get_catalogue_items",
      { source: "rawg" },
    );
    await runInvokeCase(
      () => catalogueApi.search("Arcadia"),
      "search_catalogue",
      { query: "Arcadia", source: null },
    );
    await runInvokeCase(
      () =>
        catalogueApi.upsertItem({
          id: 1,
          rawgId: 1101,
          name: "Arcadia",
          payload: "{\"source\":\"rawg\"}",
          source: "rawg",
        }),
      "upsert_catalogue_item",
      {
        input: {
          id: 1,
          rawg_id: 1101,
          name: "Arcadia",
          payload: "{\"source\":\"rawg\"}",
          source: "rawg",
        },
      },
    );
    await runInvokeCase(
      () => catalogueApi.deleteItem(1),
      "delete_catalogue_item",
      { id: 1 },
    );
    await runInvokeCase(
      () => catalogueApi.syncLibrary(),
      "sync_library_to_catalogue",
    );
  });
});

describe("settingsApi", () => {
  it("dispatches settings commands", async () => {
    await runInvokeCase(() => settingsApi.getAll(), "get_all_settings");
    await runInvokeCase(
      () => settingsApi.update(baseSettings),
      "update_settings",
      { settings: baseSettings },
    );
    await runInvokeCase(() => settingsApi.get("theme"), "get_setting", {
      key: "theme",
    });
    await runInvokeCase(() => settingsApi.set("theme", "dark"), "set_setting", {
      key: "theme",
      value: "dark",
    });
    await runInvokeCase(
      () => settingsApi.addScanDirectory("C:\\Games"),
      "add_scan_directory",
      { path: "C:\\Games" },
    );
    await runInvokeCase(
      () => settingsApi.getScanDirectories(),
      "get_scan_directories",
    );
    await runInvokeCase(
      () => settingsApi.removeScanDirectory("C:\\Games"),
      "remove_scan_directory",
      { path: "C:\\Games" },
    );
  });
});

describe("statsApi", () => {
  it("dispatches stats commands", async () => {
    await runInvokeCase(
      () => statsApi.getPlaytimeStats("2024-01-01", "2024-01-31"),
      "get_playtime_stats",
      { start: "2024-01-01", end: "2024-01-31" },
    );
  });
});

describe("scanApi", () => {
  it("dispatches scan commands", async () => {
    await runInvokeCase(
      () => scanApi.getRunningProcesses(),
      "get_running_processes",
    );
  });
});

describe("systemApi", () => {
  it("dispatches system commands", async () => {
    await runInvokeCase(() => systemApi.getInfo(), "get_system_info");
    await runInvokeCase(
      () => systemApi.testDiskSpeed("C:"),
      "test_disk_speed",
      { mountPoint: "C:" },
    );
  });
});
