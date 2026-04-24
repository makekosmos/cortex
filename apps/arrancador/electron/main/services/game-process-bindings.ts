import { execute, queryAll, queryOne, runInTransaction } from "../helpers/db";
import type { DbLike } from "../helpers/shared";
import type {
  GameProcessBinding,
  GameProcessBindingMatchType,
  NewGameProcessBinding,
} from "./games/types";

type BindingRow = {
  id: number;
  game_id: string;
  match_type: string;
  match_value: string;
  normalized_value: string;
  created_at: string;
};

type PrimaryPathOwnerRow = {
  id: string;
};

function buildParameterizedList(values: readonly string[]): string {
  return values.map((_, index) => `?${index + 1}`).join(", ");
}

export function normalizeGameProcessBindingValue(
  matchType: GameProcessBindingMatchType,
  value: string,
) {
  const trimmed = value.trim();
  if (matchType === "exe_path") {
    return trimmed.replaceAll("/", "\\").toLowerCase();
  }
  return trimmed.toLowerCase();
}

function mapBindingRow(row: BindingRow): GameProcessBinding {
  return {
    id: Number(row.id),
    game_id: row.game_id,
    match_type: row.match_type as GameProcessBindingMatchType,
    match_value: row.match_value,
    created_at: row.created_at,
  };
}

export async function listGameProcessBindings(
  db: DbLike,
  gameIds?: readonly string[],
): Promise<GameProcessBinding[]> {
  if (gameIds && gameIds.length === 0) {
    return [];
  }

  if (gameIds && gameIds.length > 0) {
    const placeholders = buildParameterizedList(gameIds);
    const rows = await queryAll<BindingRow>(
      db,
      `SELECT id, game_id, match_type, match_value, normalized_value, created_at
       FROM game_process_bindings
       WHERE game_id IN (${placeholders})
       ORDER BY created_at ASC, id ASC`,
      gameIds,
    );
    return rows.map(mapBindingRow);
  }

  const rows = await queryAll<BindingRow>(
    db,
    `SELECT id, game_id, match_type, match_value, normalized_value, created_at
     FROM game_process_bindings
     ORDER BY created_at ASC, id ASC`,
  );
  return rows.map(mapBindingRow);
}

export async function listGameProcessBindingsByGameId(
  db: DbLike,
  gameIds: readonly string[],
): Promise<Map<string, GameProcessBinding[]>> {
  const grouped = new Map<string, GameProcessBinding[]>();
  for (const gameId of gameIds) {
    grouped.set(gameId, []);
  }

  const bindings = await listGameProcessBindings(db, gameIds);
  for (const binding of bindings) {
    const current = grouped.get(binding.game_id);
    if (current) {
      current.push(binding);
      continue;
    }
    grouped.set(binding.game_id, [binding]);
  }

  return grouped;
}

async function findPrimaryExePathOwner(
  db: DbLike,
  normalizedExePath: string,
): Promise<string | null> {
  const row = await queryOne<PrimaryPathOwnerRow>(
    db,
    `SELECT id
     FROM games
     WHERE LOWER(REPLACE(exe_path, '/', '\\')) = ?1
     LIMIT 1`,
    [normalizedExePath],
  );
  return row?.id ?? null;
}

async function findBindingOwner(
  db: DbLike,
  matchType: GameProcessBindingMatchType,
  normalizedValue: string,
): Promise<string | null> {
  const row = await queryOne<{ game_id: string }>(
    db,
    `SELECT game_id
     FROM game_process_bindings
     WHERE match_type = ?1
       AND normalized_value = ?2
     LIMIT 1`,
    [matchType, normalizedValue],
  );
  return row?.game_id ?? null;
}

async function findPrimaryNormalizedExePath(
  db: DbLike,
  gameId: string,
): Promise<string> {
  const row = await queryOne<{ normalized_path: string }>(
    db,
    `SELECT LOWER(REPLACE(exe_path, '/', '\\')) AS normalized_path
     FROM games
     WHERE id = ?1`,
    [gameId],
  );
  if (!row?.normalized_path) {
    throw new Error("Game not found");
  }
  return row.normalized_path;
}

function dedupeBindings(bindings: readonly NewGameProcessBinding[]) {
  const result: NewGameProcessBinding[] = [];
  const seen = new Set<string>();

  for (const binding of bindings) {
    const normalized = normalizeGameProcessBindingValue(
      binding.match_type,
      binding.match_value,
    );
    if (!normalized) {
      continue;
    }

    const key = `${binding.match_type}:${normalized}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    result.push({
      match_type: binding.match_type,
      match_value: binding.match_value.trim(),
    });
  }

  return result;
}

export async function addGameProcessBindings(
  db: DbLike,
  gameId: string,
  bindings: readonly NewGameProcessBinding[],
  now: () => Date = () => new Date(),
): Promise<void> {
  const pending = dedupeBindings(bindings);
  if (pending.length === 0) {
    return;
  }

  const primaryNormalizedExePath = await findPrimaryNormalizedExePath(db, gameId);
  const inserts = [];

  for (const binding of pending) {
    const normalizedValue = normalizeGameProcessBindingValue(
      binding.match_type,
      binding.match_value,
    );
    if (!normalizedValue) {
      continue;
    }

    if (
      binding.match_type === "exe_path" &&
      normalizedValue === primaryNormalizedExePath
    ) {
      continue;
    }

    if (binding.match_type === "exe_path") {
      const primaryOwner = await findPrimaryExePathOwner(db, normalizedValue);
      if (primaryOwner && primaryOwner !== gameId) {
        throw new Error("This executable path is already the primary process for another game");
      }
    }

    const bindingOwner = await findBindingOwner(
      db,
      binding.match_type,
      normalizedValue,
    );
    if (bindingOwner && bindingOwner !== gameId) {
      throw new Error("This process binding is already attached to another game");
    }
    if (bindingOwner === gameId) {
      continue;
    }

    inserts.push({
      matchType: binding.match_type,
      matchValue: binding.match_value.trim(),
      normalizedValue,
      createdAt: now().toISOString(),
    });
  }

  if (inserts.length === 0) {
    return;
  }

  await runInTransaction(db, async (tx) => {
    for (const insert of inserts) {
      await execute(
        tx,
        `INSERT INTO game_process_bindings
          (game_id, match_type, match_value, normalized_value, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)`,
        [
          gameId,
          insert.matchType,
          insert.matchValue,
          insert.normalizedValue,
          insert.createdAt,
        ],
      );
    }
  });
}

export async function removeGameProcessBinding(
  db: DbLike,
  gameId: string,
  bindingId: number,
): Promise<void> {
  const row = await queryOne<{ id: number }>(
    db,
    `SELECT id
     FROM game_process_bindings
     WHERE id = ?1
       AND game_id = ?2`,
    [bindingId, gameId],
  );
  if (!row) {
    throw new Error("Process binding not found");
  }

  await execute(
    db,
    "DELETE FROM game_process_bindings WHERE id = ?1 AND game_id = ?2",
    [bindingId, gameId],
  );
}
