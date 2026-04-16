import { createRequire } from "node:module";

export interface SqliteDatabase {
  all<T = Record<string, unknown>>(sql: string, params?: readonly unknown[]): T[];
  get<T = Record<string, unknown> | undefined>(
    sql: string,
    params?: readonly unknown[],
  ): T | undefined;
  close?(): void;
}

export function openSqliteDatabase(
  filePath: string,
  options: {
    readonly?: boolean;
    fileMustExist?: boolean;
    timeoutMs?: number;
  } = {},
): SqliteDatabase {
  const require = createRequire(import.meta.url);
  const BetterSqlite3 = require("better-sqlite3") as new (
    path: string,
    options?: {
      readonly?: boolean;
      fileMustExist?: boolean;
      timeout?: number;
    },
  ) => {
    prepare(sql: string): {
      all<T = Record<string, unknown>>(...params: unknown[]): T[];
      get<T = Record<string, unknown> | undefined>(...params: unknown[]): T;
    };
    close(): void;
  };

  const nativeDb = new BetterSqlite3(filePath, {
    ...(typeof options.readonly === "boolean"
      ? { readonly: options.readonly }
      : {}),
    ...(typeof options.fileMustExist === "boolean"
      ? { fileMustExist: options.fileMustExist }
      : {}),
    ...(typeof options.timeoutMs === "number" ? { timeout: options.timeoutMs } : {}),
  });

  const prepare = (sql: string) => nativeDb.prepare(sql.replace(/\?\d+/g, "?"));

  return {
    all<T = Record<string, unknown>>(sql: string, params: readonly unknown[] = []): T[] {
      const statement = prepare(sql);
      return (params.length === 0
        ? statement.all()
        : statement.all(params as unknown[])) as T[];
    },
    get<T = Record<string, unknown> | undefined>(
      sql: string,
      params: readonly unknown[] = [],
    ): T | undefined {
      const statement = prepare(sql);
      return (params.length === 0
        ? statement.get()
        : statement.get(params as unknown[])) as T | undefined;
    },
    close() {
      nativeDb.close();
    },
  };
}
