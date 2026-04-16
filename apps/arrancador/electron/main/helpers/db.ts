import type { DbLike, DbValue, MaybePromise } from "./shared";

export async function runInTransaction<T>(
  db: DbLike,
  fn: (tx: DbLike) => MaybePromise<T>,
): Promise<T> {
  if (db.transaction) {
    return await Promise.resolve(db.transaction(fn));
  }

  return await Promise.resolve(fn(db));
}

export async function queryAll<T>(
  db: DbLike,
  sql: string,
  params: readonly DbValue[] = [],
): Promise<T[]> {
  return await Promise.resolve(db.all<T>(sql, params));
}

export async function queryOne<T>(
  db: DbLike,
  sql: string,
  params: readonly DbValue[] = [],
): Promise<T | undefined> {
  return await Promise.resolve(db.get<T>(sql, params));
}

export async function execute(
  db: DbLike,
  sql: string,
  params: readonly DbValue[] = [],
): Promise<number> {
  const result = await Promise.resolve(db.run(sql, params));
  return result.changes;
}

