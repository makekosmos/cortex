/**
 * Delphi StorageBackend — implements @arksync/core StorageBackend
 * using the Rust sidecar (delphi-db) for persistence.
 */

import type { StorageBackend } from '@arksync/core';
import type { SyncEntity, VersionVector } from '@arksync/core';
import { HLC } from '@arksync/core';
import {
  dbLoadAll,
  dbUpsertTodo,
  dbDeleteTodo,
  dbUpsertProject,
  dbDeleteProject,
  dbUpsertArea,
  dbUpsertTag,
  dbUpsertHeading,
  dbDeleteHeading,
  dbGetSyncKv,
  dbSetSyncKv,
  type TodoItem,
  type Project,
  type Area,
  type Tag,
  type Heading,
} from './sidecar';

export class DelphiStorage implements StorageBackend {
  constructor(private deviceId: string) {}

  async loadEntities(vector: VersionVector): Promise<SyncEntity[]> {
    const data = await dbLoadAll();
    const entities: SyncEntity[] = [];

    for (const todo of data.todos) {
      entities.push({
        type: 'todo',
        id: todo.id,
        data: todo as unknown as Record<string, unknown>,
        hlc: vector[todo.id] || HLC.now(this.deviceId).toString(),
      });
    }

    for (const project of data.projects) {
      entities.push({
        type: 'project',
        id: project.id,
        data: project as unknown as Record<string, unknown>,
        hlc: vector[project.id] || HLC.now(this.deviceId).toString(),
      });
    }

    for (const area of data.areas) {
      entities.push({
        type: 'area',
        id: area.id,
        data: area as unknown as Record<string, unknown>,
        hlc: vector[area.id] || HLC.now(this.deviceId).toString(),
      });
    }

    for (const tag of data.tags) {
      entities.push({
        type: 'tag',
        id: tag.id,
        data: tag as unknown as Record<string, unknown>,
        hlc: vector[tag.id] || HLC.now(this.deviceId).toString(),
      });
    }

    for (const heading of data.headings) {
      entities.push({
        type: 'heading',
        id: heading.id,
        data: heading as unknown as Record<string, unknown>,
        hlc: vector[heading.id] || HLC.now(this.deviceId).toString(),
      });
    }

    return entities;
  }

  async applyEntity(entity: SyncEntity): Promise<void> {
    try {
      if (entity.deleted) {
        switch (entity.type) {
          case 'todo':    await dbDeleteTodo(entity.id); break;
          case 'project': await dbDeleteProject(entity.id); break;
          case 'heading': await dbDeleteHeading(entity.id); break;
        }
        return;
      }

      switch (entity.type) {
        case 'todo':    await dbUpsertTodo(entity.data as unknown as TodoItem); break;
        case 'project': await dbUpsertProject(entity.data as unknown as Project); break;
        case 'area':    await dbUpsertArea(entity.data as unknown as Area); break;
        case 'tag':     await dbUpsertTag(entity.data as unknown as Tag); break;
        case 'heading': await dbUpsertHeading(entity.data as unknown as Heading); break;
      }
    } catch (err) {
      console.error(`[DelphiStorage] Failed to apply entity ${entity.type}/${entity.id}:`, err);
    }
  }

  async getKv(key: string): Promise<string | null> {
    return dbGetSyncKv(key);
  }

  async setKv(key: string, value: string): Promise<void> {
    return dbSetSyncKv(key, value);
  }
}
