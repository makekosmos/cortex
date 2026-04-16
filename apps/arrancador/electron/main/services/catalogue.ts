import {
  buildLibraryPayload,
  type CatalogueItemRow,
  createCatalogueSyncResult,
  mapCatalogueRow,
  normalizeCatalogueSource,
  stableCatalogueRawgId,
  type UpsertCatalogueItemInput,
  validateCataloguePayload,
} from "../helpers/catalogue";
import { execute, queryAll, queryOne, runInTransaction } from "../helpers/db";
import type {
  CatalogueItem,
  CatalogueLibraryPort,
  CatalogueSyncResult,
  DbLike,
} from "../helpers/shared";

export interface CatalogueServiceDeps {
  db: DbLike;
  library: CatalogueLibraryPort;
}

export interface CatalogueService {
  getCatalogueItems(source?: string | null): Promise<CatalogueItem[]>;
  searchCatalogue(query: string, source?: string | null): Promise<CatalogueItem[]>;
  upsertCatalogueItem(input: UpsertCatalogueItemInput): Promise<CatalogueItem>;
  deleteCatalogueItem(id: number): Promise<boolean>;
  syncLibraryToCatalogue(): Promise<CatalogueSyncResult>;
}

async function listCatalogueRows(
  db: DbLike,
  source?: string | null,
): Promise<CatalogueItem[]> {
  if (source) {
    const rows = await queryAll<CatalogueItemRow>(
      db,
      `SELECT id, rawg_id, name, payload, source, updated_at
       FROM catalogue_items
       WHERE source = ?1
       ORDER BY name ASC`,
      [source],
    );
    return rows.map(mapCatalogueRow);
  }

  const rows = await queryAll<CatalogueItemRow>(
    db,
    `SELECT id, rawg_id, name, payload, source, updated_at
     FROM catalogue_items
     ORDER BY name ASC`,
  );
  return rows.map(mapCatalogueRow);
}

export function createCatalogueService(deps: CatalogueServiceDeps): CatalogueService {
  return {
    async getCatalogueItems(source?: string | null): Promise<CatalogueItem[]> {
      return await listCatalogueRows(deps.db, source);
    },

    async searchCatalogue(query: string, source?: string | null): Promise<CatalogueItem[]> {
      const trimmed = query.trim();
      if (trimmed.length === 0) {
        return await listCatalogueRows(deps.db, source);
      }

      const pattern = `%${trimmed}%`;
      if (source) {
        const rows = await queryAll<CatalogueItemRow>(
          deps.db,
          `SELECT id, rawg_id, name, payload, source, updated_at
           FROM catalogue_items
           WHERE source = ?1 AND (name LIKE ?2 OR payload LIKE ?2)
           ORDER BY name ASC`,
          [source, pattern],
        );
        return rows.map(mapCatalogueRow);
      }

      const rows = await queryAll<CatalogueItemRow>(
        deps.db,
        `SELECT id, rawg_id, name, payload, source, updated_at
         FROM catalogue_items
         WHERE name LIKE ?1 OR payload LIKE ?1
         ORDER BY name ASC`,
        [pattern],
      );
      return rows.map(mapCatalogueRow);
    },

    async upsertCatalogueItem(input: UpsertCatalogueItemInput): Promise<CatalogueItem> {
      const name = input.name.trim();
      if (name.length === 0) {
        throw new Error("Name cannot be empty");
      }

      validateCataloguePayload(input.payload);
      const source = normalizeCatalogueSource(input.source);

      if (typeof input.id === "number") {
        const updatedAt = new Date().toISOString();
        const changes = await execute(
          deps.db,
          `UPDATE catalogue_items
           SET rawg_id = ?1, name = ?2, payload = ?3, source = ?4, updated_at = ?5
           WHERE id = ?6`,
          [input.rawgId, name, input.payload, source, updatedAt, input.id],
        );

        if (changes === 0) {
          throw new Error("Catalogue item not found");
        }

        const row = await queryOne<CatalogueItemRow>(
          deps.db,
          `SELECT id, rawg_id, name, payload, source, updated_at
           FROM catalogue_items
           WHERE id = ?1`,
          [input.id],
        );

        if (!row) {
          throw new Error("Catalogue item not found");
        }

        return mapCatalogueRow(row);
      }

      const updatedAt = new Date().toISOString();
      await execute(
        deps.db,
        `INSERT INTO catalogue_items (rawg_id, name, payload, source, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(rawg_id, source) DO UPDATE
         SET name = excluded.name,
             payload = excluded.payload,
             updated_at = excluded.updated_at`,
        [input.rawgId, name, input.payload, source, updatedAt],
      );

      const row = await queryOne<CatalogueItemRow>(
        deps.db,
        `SELECT id, rawg_id, name, payload, source, updated_at
         FROM catalogue_items
         WHERE rawg_id = ?1 AND source = ?2
         LIMIT 1`,
        [input.rawgId, source],
      );

      if (!row) {
        throw new Error("Failed to load upserted catalogue item");
      }

      return mapCatalogueRow(row);
    },

    async deleteCatalogueItem(id: number): Promise<boolean> {
      return (await execute(deps.db, "DELETE FROM catalogue_items WHERE id = ?1", [id])) > 0;
    },

    async syncLibraryToCatalogue(): Promise<CatalogueSyncResult> {
      const libraryItems = await deps.library.listGames();
      const updatedAt = new Date().toISOString();
      const source = "library";

      await runInTransaction(deps.db, async (tx) => {
        for (const item of libraryItems) {
          const payload = buildLibraryPayload(item);
          const rawgId = stableCatalogueRawgId(item.id);

          await execute(
            tx,
            `INSERT INTO catalogue_items (rawg_id, name, payload, source, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(rawg_id, source) DO UPDATE
             SET name = excluded.name,
                 payload = excluded.payload,
                 updated_at = excluded.updated_at`,
            [rawgId, item.name, payload, source, updatedAt],
          );
        }
      });

      return createCatalogueSyncResult(libraryItems.length);
    },
  };
}
