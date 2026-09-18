import fs from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import type { SQLOutputValue } from "node:sqlite";
import type { ChildProcess } from "node:child_process";
import { test, expect } from "@playwright/test";
import type { ElectronApplication } from "playwright";
import {
  cleanupManifest,
  closeHost,
  engineBinaries,
  launchManager,
  managerE2eRoot,
  managerEnvironment,
  managerMain,
  recordCleanup,
  rpc,
  rpcError,
  startEngine,
  terminate,
  waitForPidGone,
} from "./manager-runtime";
import type { JsonValue } from "./manager-runtime";

const isJsonObject = (
  value: JsonValue | undefined,
): value is { readonly [key: string]: JsonValue } =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const isString = (value: JsonValue | undefined): value is string => typeof value === "string";
const isSqlText = (value: SQLOutputValue | undefined): value is string => typeof value === "string";

// `kepler.last_backup_ts` пишется в sync_kv внутри ark.db ПОСЛЕ дампа
// (`run_backup_now` ждёт завершения копии и только потом пишет ts). Поэтому
// каждый снапшот содержит состояние ДО собственного ts — прямое чтение
// sync_kv доказывает, что restore подменил живую базу содержимым снимка.
const SYNC_KV_LAST_BACKUP = "kepler.last_backup_ts";

const readSyncKv = (dbPath: string, key: string): string | null => {
  const db = new DatabaseSync(dbPath);
  try {
    const row = db.prepare("SELECT value FROM sync_kv WHERE key = ?").get(key);
    const value = row?.value;
    return isSqlText(value) ? value : null;
  } finally {
    db.close();
  }
};

const backupIds = async (lock: Parameters<typeof rpc>[0]): Promise<string[]> => {
  const listed = await rpc(lock, "manager.db_backups.list");
  expect(listed.ok, rpcError(listed)).toBe(true);
  const backups = isJsonObject(listed.data) ? listed.data.backups : undefined;
  if (!Array.isArray(backups)) {
    throw new Error("db_backups.list returned no backups array");
  }
  return backups
    .map((entry) => (isJsonObject(entry) ? entry.id : undefined))
    .filter(isString)
    .sort();
};

const seedCorruptSnapshot = (dataDir: string, name: string): string => {
  const target = path.join(dataDir, "backups", name);
  fs.writeFileSync(target, Buffer.from("not a sqlite snapshot"));
  return name;
};

test("Manager restores a database snapshot through Core RPCs and reloads", async () => {
  test.skip(!fs.existsSync(managerMain), `build Manager first: ${managerMain}`);

  const runRoot = managerE2eRoot("db-restore");
  const dataDir = path.join(runRoot, "data");
  const binaries = engineBinaries();
  const pids = new Set<number>();
  const managerEnv = managerEnvironment(runRoot, dataDir);
  const engineEnv = { KOSMOS_TEST_MODE: "1", RUST_LOG: "error" };
  const dbPath = path.join(dataDir, "ark.db");
  let engine: ChildProcess | undefined;
  let manager: ElectronApplication | undefined;
  try {
    const started = await startEngine(binaries.engine, binaries.ark, dataDir, engineEnv);
    engine = started.child;
    if (engine.pid) pids.add(engine.pid);
    const lock = started.lock;

    const first = await rpc(lock, "manager.db_backups.create");
    expect(first.ok, rpcError(first)).toBe(true);
    // Имя снапшота имеет секундное разрешение — второй create в ту же
    // секунду перезаписал бы файл первого.
    await new Promise((resolve) => setTimeout(resolve, 1100));
    const second = await rpc(lock, "manager.db_backups.create");
    expect(second.ok, rpcError(second)).toBe(true);

    // Live db carries the post-dump timestamp; the snapshots do not contain
    // the value that was written after their own dump completed.
    const tsLive = readSyncKv(dbPath, SYNC_KV_LAST_BACKUP);
    expect(tsLive).toBeTruthy();

    const ids = await backupIds(lock);
    expect(ids.length).toBeGreaterThanOrEqual(2);
    const oldest = ids[0];

    manager = await launchManager(runRoot, "restore", managerEnv);
    if (manager.process().pid) pids.add(manager.process().pid);
    const page = await manager.firstWindow();
    page.on("dialog", (dialog) => void dialog.accept());
    await page.getByRole("button", { name: "Настройки" }).click();
    await expect(page.getByRole("heading", { name: "Резервные копии базы" })).toBeVisible();

    const row = page.locator(".settings-row").filter({
      hasText: oldest,
    });
    await expect(row.first()).toBeVisible();
    await row.first().getByRole("button", { name: "Восстановить" }).click();
    await expect(page.getByRole("status")).toContainText(/Восстановлено/);

    // KOS-77: после успешного restore Manager сам перезагружает окно, чтобы
    // не показывать устаревшее in-memory состояние.
    await page.waitForEvent("framenavigated", { timeout: 15_000 });

    // База подменена содержимым снимка: ts, записанный после дампа, исчез.
    const tsRestored = readSyncKv(dbPath, SYNC_KV_LAST_BACKUP);
    expect(tsRestored).not.toBe(tsLive);
  } finally {
    const cleanupErrors: unknown[] = [];
    const attempt = async (action: () => Promise<void>) => {
      try {
        await action();
      } catch (error) {
        cleanupErrors.push(error);
      }
    };
    await attempt(() => closeHost(manager, pids));
    await attempt(() => terminate(engine, binaries.engine, dataDir, "Engine"));
    for (const pid of pids) {
      await attempt(() => waitForPidGone(pid, "recorded teardown process"));
    }
    try {
      recordCleanup(cleanupManifest(), runRoot, pids);
    } catch (error) {
      cleanupErrors.push(error);
    }
    expect(cleanupErrors, "db restore E2E cleanup failed").toHaveLength(0);
  }
});

test("Corrupt and foreign snapshots fail closed without touching the live db", async () => {
  test.skip(!fs.existsSync(managerMain), `build Manager first: ${managerMain}`);

  const runRoot = managerE2eRoot("db-restore-corrupt");
  const dataDir = path.join(runRoot, "data");
  const binaries = engineBinaries();
  const pids = new Set<number>();
  const managerEnv = managerEnvironment(runRoot, dataDir);
  const engineEnv = { KOSMOS_TEST_MODE: "1", RUST_LOG: "error" };
  const dbPath = path.join(dataDir, "ark.db");
  let engine: ChildProcess | undefined;
  let manager: ElectronApplication | undefined;
  try {
    const started = await startEngine(binaries.engine, binaries.ark, dataDir, engineEnv);
    engine = started.child;
    if (engine.pid) pids.add(engine.pid);
    const lock = started.lock;

    const created = await rpc(lock, "manager.db_backups.create");
    expect(created.ok, rpcError(created)).toBe(true);
    const tsLive = readSyncKv(dbPath, SYNC_KV_LAST_BACKUP);
    expect(tsLive).toBeTruthy();

    // Битый файл с валидным basename проходит в список, но обязан быть
    // отклонён validate/restore без изменения живой базы.
    const corrupt = seedCorruptSnapshot(dataDir, "ark.db.backup-2020-01-01-000000");
    const validated = await rpc(lock, "manager.db_backups.validate", {
      backup_id: corrupt,
    });
    expect(validated.ok, rpcError(validated)).toBe(true);
    const verdict = isJsonObject(validated.data) ? validated.data : {};
    expect(verdict.exists).toBe(true);
    expect(verdict.valid).not.toBe(true);

    const restored = await rpc(lock, "manager.db_backups.restore", {
      backup_id: corrupt,
    });
    expect(restored.ok).toBe(false);
    expect(readSyncKv(dbPath, SYNC_KV_LAST_BACKUP)).toBe(tsLive);

    // Чужой файл с чистым basename виден в списке Core (list не требует
    // `ark.db.backup-*` паттерна), но engine-boundary отклоняет любые
    // validate/restore для него до RPC — fail closed.
    fs.writeFileSync(path.join(dataDir, "backups", "stranger.db"), Buffer.from("foreign bytes"));
    const foreignValidate = await rpc(lock, "manager.db_backups.validate", {
      backup_id: "stranger.db",
    });
    expect(foreignValidate.ok).toBe(false);
    const foreignRestore = await rpc(lock, "manager.db_backups.restore", {
      backup_id: "stranger.db",
    });
    expect(foreignRestore.ok).toBe(false);
    expect(readSyncKv(dbPath, SYNC_KV_LAST_BACKUP)).toBe(tsLive);

    manager = await launchManager(runRoot, "corrupt", managerEnv);
    if (manager.process().pid) pids.add(manager.process().pid);
    const page = await manager.firstWindow();
    page.on("dialog", (dialog) => void dialog.accept());
    await page.getByRole("button", { name: "Настройки" }).click();
    await expect(page.getByRole("heading", { name: "Резервные копии базы" })).toBeVisible();

    const corruptRow = page.locator(".settings-row").filter({ hasText: corrupt });
    await expect(corruptRow.first()).toBeVisible();
    await corruptRow.first().getByRole("button", { name: "Проверить" }).click();
    await expect(page.getByRole("status")).toContainText(/не годен/);
    expect(readSyncKv(dbPath, SYNC_KV_LAST_BACKUP)).toBe(tsLive);
  } finally {
    const cleanupErrors: unknown[] = [];
    const attempt = async (action: () => Promise<void>) => {
      try {
        await action();
      } catch (error) {
        cleanupErrors.push(error);
      }
    };
    await attempt(() => closeHost(manager, pids));
    await attempt(() => terminate(engine, binaries.engine, dataDir, "Engine"));
    for (const pid of pids) {
      await attempt(() => waitForPidGone(pid, "recorded teardown process"));
    }
    try {
      recordCleanup(cleanupManifest(), runRoot, pids);
    } catch (error) {
      cleanupErrors.push(error);
    }
    expect(cleanupErrors, "db restore E2E cleanup failed").toHaveLength(0);
  }
});
