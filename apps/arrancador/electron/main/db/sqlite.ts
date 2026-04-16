import { createRequire } from "node:module";

import type { DbLike, DbRunResult, DbValue, MaybePromise } from "../helpers/shared";

export type SqliteDatabase = DbLike;

export interface OpenSqliteDatabaseOptions {
  readonly?: boolean;
  fileMustExist?: boolean;
  timeoutMs?: number;
  verbose?: (sql: string) => void;
}

export function openSqliteDatabase(
  filePath: string,
  options: OpenSqliteDatabaseOptions = {},
): SqliteDatabase {
  const require = createRequire(import.meta.url);
  const BetterSqlite3 = require("better-sqlite3") as new (
    path: string,
    options?: {
      readonly?: boolean;
      fileMustExist?: boolean;
      timeout?: number;
      verbose?: (sql: string) => void;
    } | undefined,
  ) => {
    prepare(sql: string): {
      all<T = Record<string, unknown>>(...params: DbValue[]): T[];
      get<T = Record<string, unknown> | undefined>(...params: DbValue[]): T;
      run(...params: DbValue[]): DbRunResult;
    };
    exec(sql: string): void;
    close?(): void;
  };

  const nativeDb = new BetterSqlite3(filePath, {
    ...(typeof options.readonly === "boolean"
      ? { readonly: options.readonly }
      : {}),
    ...(typeof options.fileMustExist === "boolean"
      ? { fileMustExist: options.fileMustExist }
      : {}),
    ...(typeof options.timeoutMs === "number"
      ? { timeout: options.timeoutMs }
      : {}),
    ...(typeof options.verbose === "function"
      ? { verbose: options.verbose }
      : {}),
  });

  const prepare = (sql: string) => nativeDb.prepare(sql.replace(/\?\d+/g, "?"));

  let wrapper: SqliteDatabase;
  wrapper = {
    all<T = Record<string, unknown>>(
      sql: string,
      params: readonly DbValue[] = [],
    ): T[] {
      const statement = prepare(sql);
      return (params.length === 0
        ? statement.all()
        : statement.all(params as unknown as DbValue[])) as T[];
    },

    get<T = Record<string, unknown> | undefined>(
      sql: string,
      params: readonly DbValue[] = [],
    ): T | undefined {
      const statement = prepare(sql);
      return (params.length === 0
        ? statement.get()
        : statement.get(params as unknown as DbValue[])) as T | undefined;
    },

    run(sql: string, params: readonly DbValue[] = []): DbRunResult {
      const statement = prepare(sql);
      return (params.length === 0
        ? statement.run()
        : statement.run(params as unknown as DbValue[])) as DbRunResult;
    },

    async transaction<T>(fn: (tx: DbLike) => MaybePromise<T>): Promise<T> {
      nativeDb.exec("BEGIN IMMEDIATE TRANSACTION;");
      try {
        const result = await Promise.resolve(fn(wrapper));
        nativeDb.exec("COMMIT;");
        return result;
      } catch (error) {
        try {
          nativeDb.exec("ROLLBACK;");
        } catch {
          // Ignore rollback failures so the original error is preserved.
        }
        throw error;
      }
    },
  };

  return wrapper;
}

export async function withTransaction<T>(
  db: SqliteDatabase,
  fn: () => MaybePromise<T>,
): Promise<T> {
  if (db.transaction) {
    return await Promise.resolve(db.transaction(async () => await Promise.resolve(fn())));
  }

  return await Promise.resolve(fn());
}

export async function enableForeignKeys(db: SqliteDatabase): Promise<void> {
  await Promise.resolve(db.run("PRAGMA foreign_keys = ON;"));
}
