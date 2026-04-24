import { execute, queryAll, queryOne } from "../helpers/db";
import type { DbLike } from "../helpers/shared";

export async function getSettingsMap(db: DbLike): Promise<Record<string, string>> {
  const rows = await queryAll<{ key: string; value: string }>(
    db,
    "SELECT key, value FROM settings",
  );

  return Object.fromEntries(rows.map((row) => [row.key, row.value]));
}

export async function getSetting(db: DbLike, key: string): Promise<string | null> {
  const row = await queryOne<{ value: string }>(
    db,
    "SELECT value FROM settings WHERE key = ?1",
    [key],
  );
  return row?.value ?? null;
}

export async function setSetting(
  db: DbLike,
  key: string,
  value: string,
): Promise<void> {
  await execute(
    db,
    "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
    [key, value],
  );
}
