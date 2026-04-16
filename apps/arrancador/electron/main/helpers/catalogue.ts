import type { CatalogueItem, CatalogueLibraryItem, CatalogueSyncResult } from "./shared";

const FNV64_OFFSET_BASIS = 0xcbf29ce484222325n;
const FNV64_PRIME = 0x100000001b3n;
const SAFE_ID_MASK = 0x1fffffffffffffn;

export interface CatalogueItemRow {
  id: number;
  rawg_id: number;
  name: string;
  payload: string;
  source: string;
  updated_at: string;
}

export interface UpsertCatalogueItemInput {
  id?: number;
  rawgId: number;
  name: string;
  payload: string;
  source?: string | null;
}

export function normalizeCatalogueSource(source: string | null | undefined): string {
  const normalized = source?.trim();
  return normalized && normalized.length > 0 ? normalized : "rawg";
}

export function validateCataloguePayload(payload: string): void {
  if (typeof payload !== "string") {
    throw new Error("Payload must be valid JSON");
  }

  try {
    JSON.parse(payload);
  } catch {
    throw new Error("Payload must be valid JSON");
  }
}

export function stableCatalogueRawgId(value: string): number {
  const bytes = new TextEncoder().encode(value);
  let hash = FNV64_OFFSET_BASIS;

  for (const byte of bytes) {
    hash ^= BigInt(byte);
    hash = (hash * FNV64_PRIME) & 0xffffffffffffffffn;
  }

  const safe = Number(hash & SAFE_ID_MASK);
  return safe > 0 ? safe : 1;
}

export function mapCatalogueRow(row: CatalogueItemRow): CatalogueItem {
  return {
    id: row.id,
    rawg_id: row.rawg_id,
    name: row.name,
    payload: row.payload,
    source: row.source,
    updated_at: row.updated_at,
  };
}

export function buildLibraryPayload(item: CatalogueLibraryItem): string {
  return JSON.stringify({
    game_id: item.id,
    exe_name: item.exe_name,
  });
}

export function createCatalogueSyncResult(count: number): CatalogueSyncResult {
  return { synced: count };
}

