/**
 * ArkSync StorageBackend — abstract interface for app-specific persistence.
 *
 * Each application implements this interface to wire up its own database
 * (Room, SQLite sidecar, localStorage, etc.) to the generic sync engine.
 */

import type { SyncEntity, VersionVector } from "./protocol";

export interface StorageBackend {
  /** Load all entities for initial sync. Called during version_vector exchange. */
  loadEntities(vector: VersionVector): Promise<SyncEntity[]>;

  /** Apply a received entity (upsert or delete). Called for each incoming change. */
  applyEntity(entity: SyncEntity): Promise<void>;

  /** Read a key-value pair (used for version vector, known peers, etc.). */
  getKv(key: string): Promise<string | null>;

  /** Write a key-value pair. */
  setKv(key: string, value: string): Promise<void>;
}
