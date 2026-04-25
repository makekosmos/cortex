import fs from "node:fs/promises";
import path from "node:path";

import {
  noteTypeSchema,
  parseNoteTypeDefinition,
  parseNoteTypeUiSchema,
  type NoteType,
} from "../src/lib/typedNotes.ts";

export interface Entry {
  id: string;
  title: string;
  content_json: string;
  created_at: number;
  updated_at: number;
  folder_id: string | null;
  type_id: string | null;
  header_layout: string | null;
  header_props_json: string;
  schema_version: number;
  deleted_at: number | null;
}

export interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string;
  contentJson: unknown;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
}

export interface ArkObjectTypeRecord {
  id: string;
  name: string;
  schemaJson: string;
  uiSchemaJson: string;
  createdAt: string;
  updatedAt: string;
  systemLocked: boolean;
}

export interface ArkObjectLinkRecord {
  id: string;
  sourceObjectId: string;
  targetObjectId: string;
  linkType: string;
  createdAt: string;
}

export interface EdenArkObjectMigrationDeps {
  listNoteTypes(vaultPath: string): Promise<NoteType[]>;
  listEntries(vaultPath: string): Promise<Entry[]>;
  listObjects(): Promise<ArkObjectRecord[]>;
  listObjectLinks(): Promise<ArkObjectLinkRecord[]>;
  upsertObjectType(objectType: ArkObjectTypeRecord): Promise<boolean>;
  upsertObject(object: ArkObjectRecord): Promise<boolean>;
  upsertObjectLink(objectLink: ArkObjectLinkRecord): Promise<boolean>;
  deleteObjectLink(id: string): Promise<boolean>;
}

const DEFAULT_ARK_TYPE_ID = "note_obj";
const EDEN_NOTE_OBJECT_MIGRATION_ID = "eden-note-objects-v1";

export interface EdenMigrationRecordError {
  id: string;
  phase: "object_type" | "object" | "link";
  message: string;
}

export interface EdenArkObjectMigrationReport {
  migrationId: string;
  startedAt: string;
  finishedAt: string;
  status: "success" | "partial_failure" | "failed" | "noop";
  typeSourceCount: number;
  typeMigratedCount: number;
  typeFailedCount: number;
  objectSourceCount: number;
  objectMigratedCount: number;
  objectSkippedCount: number;
  objectFailedCount: number;
  linkMigratedCount: number;
  linkDeletedCount: number;
  linkFailedCount: number;
  backupPath: string | null;
  errors: EdenMigrationRecordError[];
}

export interface EdenArkObjectMigrationOptions {
  backupDir?: string | null;
  now?: () => string;
}

async function writeMigrationBackup(
  backupDir: string | null | undefined,
  snapshot: unknown,
): Promise<string | null> {
  if (!backupDir) {
    return null;
  }

  await fs.mkdir(backupDir, { recursive: true });
  const backupPath = path.join(backupDir, `${EDEN_NOTE_OBJECT_MIGRATION_ID}.json`);
  try {
    await fs.writeFile(backupPath, JSON.stringify(snapshot, null, 2), { flag: "wx" });
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") {
      throw error;
    }
  }
  return backupPath;
}

function buildReportStatus(report: EdenArkObjectMigrationReport) {
  const failures = report.typeFailedCount + report.objectFailedCount + report.linkFailedCount;
  const writes =
    report.typeMigratedCount +
    report.objectMigratedCount +
    report.linkMigratedCount +
    report.linkDeletedCount;
  if (report.typeSourceCount === 0 && report.objectSourceCount === 0) {
    return "noop" as const;
  }
  if (failures === 0) {
    return "success" as const;
  }
  if (writes > 0) {
    return "partial_failure" as const;
  }
  return "failed" as const;
}

function millisToArkTimestamp(value: number | null | undefined) {
  const timestamp = typeof value === "number" ? value : Date.now();
  return new Date(timestamp).toISOString();
}

function parseHeaderPropsJson(raw: string | null | undefined) {
  if (!raw?.trim()) {
    return {} as Record<string, unknown>;
  }
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function normalizeEntry(entry: Entry): Entry {
  return {
    ...entry,
    type_id: entry.type_id ?? null,
    header_layout: entry.header_layout ?? null,
    header_props_json: entry.header_props_json ?? "{}",
    schema_version: entry.schema_version ?? 1,
    deleted_at: entry.deleted_at ?? null,
  };
}

function normalizeNoteType(noteType: NoteType) {
  return noteTypeSchema.parse({
    ...noteType,
    ui_schema_json: noteType.ui_schema_json ?? JSON.stringify({}),
  });
}

function mapEntryToArkObject(entry: Entry): ArkObjectRecord {
  const headerProps = parseHeaderPropsJson(entry.header_props_json);
  const { related_notes: _relatedNotes, ...propsJson } = headerProps;
  let contentJson: unknown = { type: "doc", content: [{ type: "paragraph" }] };
  try {
    contentJson = JSON.parse(entry.content_json || JSON.stringify(contentJson));
  } catch {}

  return {
    id: entry.id,
    typeId: entry.type_id ?? DEFAULT_ARK_TYPE_ID,
    title: entry.title,
    contentJson,
    propsJson,
    createdAt: millisToArkTimestamp(entry.created_at),
    updatedAt: millisToArkTimestamp(entry.updated_at),
    deletedAt: entry.deleted_at ? millisToArkTimestamp(entry.deleted_at) : null,
  };
}

function mapNoteTypeToArkObjectType(noteType: NoteType): ArkObjectTypeRecord {
  const uiSchema = parseNoteTypeUiSchema(noteType.ui_schema_json);
  const definition = parseNoteTypeDefinition(noteType.schema_json);
  const featuredFromUi = uiSchema.featured_fields ?? [];
  const visibleFromFields = definition.fields
    .filter((field) => field.visible !== false)
    .map((field) => field.id);
  const readOnlyFromFields = definition.fields
    .filter((field) => field.read_only === true)
    .map((field) => field.id);

  return {
    id: noteType.id,
    name: noteType.name,
    schemaJson: noteType.schema_json,
    uiSchemaJson: JSON.stringify({
      featured_fields: featuredFromUi,
      visible_fields: uiSchema.visible_fields?.length ? uiSchema.visible_fields : visibleFromFields,
      hidden_fields: uiSchema.hidden_fields ?? ["created_at", "updated_at", "deleted_at"],
      read_only_fields: uiSchema.read_only_fields?.length
        ? uiSchema.read_only_fields
        : readOnlyFromFields,
      field_order:
        uiSchema.field_order?.length ? uiSchema.field_order : definition.fields.map((field) => field.id),
      header_layout: uiSchema.header_layout ?? "inline",
      default_layout: uiSchema.default_layout ?? "page",
      default_template_id: uiSchema.default_template_id ?? null,
    }),
    createdAt: millisToArkTimestamp(noteType.created_at),
    updatedAt: millisToArkTimestamp(noteType.updated_at),
    systemLocked: noteType.id === "note_obj" || noteType.id === "game_obj",
  };
}

async function syncArkObjectLinks(
  deps: Pick<EdenArkObjectMigrationDeps, "listObjectLinks" | "upsertObjectLink" | "deleteObjectLink">,
  entry: Entry,
  report: EdenArkObjectMigrationReport,
) {
  const headerProps = parseHeaderPropsJson(entry.header_props_json);
  const relatedNotes = Array.isArray(headerProps.related_notes)
    ? headerProps.related_notes.filter((value): value is string => typeof value === "string")
    : [];

  const existingLinks = await deps.listObjectLinks();
  const ownedLinks = existingLinks.filter(
    (link) => link.sourceObjectId === entry.id && link.linkType === "related",
  );

  for (const link of ownedLinks) {
    if (!relatedNotes.includes(link.targetObjectId)) {
      try {
        await deps.deleteObjectLink(link.id);
        report.linkDeletedCount += 1;
      } catch (error) {
        report.linkFailedCount += 1;
        report.errors.push({
          id: link.id,
          phase: "link",
          message: (error as Error).message,
        });
      }
    }
  }

  for (const targetObjectId of relatedNotes) {
    const existing = ownedLinks.find((link) => link.targetObjectId === targetObjectId);
    if (existing) {
      continue;
    }

    const objectLink = {
      id: `${entry.id}:related:${targetObjectId}`,
      sourceObjectId: entry.id,
      targetObjectId,
      linkType: "related",
      createdAt: millisToArkTimestamp(entry.updated_at),
    };
    try {
      await deps.upsertObjectLink(objectLink);
      report.linkMigratedCount += 1;
    } catch (error) {
      report.linkFailedCount += 1;
      report.errors.push({
        id: objectLink.id,
        phase: "link",
        message: (error as Error).message,
      });
    }
  }
}

export async function runHeartVaultToArkObjectMigration(
  deps: EdenArkObjectMigrationDeps,
  vaultPath: string,
  options: EdenArkObjectMigrationOptions = {},
): Promise<EdenArkObjectMigrationReport> {
  const now = options.now ?? (() => new Date().toISOString());
  const startedAt = now();
  const [heartTypes, heartEntries, arkObjects] = await Promise.all([
    deps.listNoteTypes(vaultPath).then((types) => types.map(normalizeNoteType)),
    deps.listEntries(vaultPath).then((entries) => entries.map(normalizeEntry)),
    deps.listObjects(),
  ]);
  const backupPath = await writeMigrationBackup(
    options.backupDir,
    {
      migrationId: EDEN_NOTE_OBJECT_MIGRATION_ID,
      createdAt: startedAt,
      vaultPath,
      noteTypes: heartTypes,
      entries: heartEntries,
    },
  );
  const report: EdenArkObjectMigrationReport = {
    migrationId: EDEN_NOTE_OBJECT_MIGRATION_ID,
    startedAt,
    finishedAt: startedAt,
    status: "noop",
    typeSourceCount: heartTypes.length,
    typeMigratedCount: 0,
    typeFailedCount: 0,
    objectSourceCount: heartEntries.length,
    objectMigratedCount: 0,
    objectSkippedCount: 0,
    objectFailedCount: 0,
    linkMigratedCount: 0,
    linkDeletedCount: 0,
    linkFailedCount: 0,
    backupPath,
    errors: [],
  };

  for (const noteType of heartTypes) {
    try {
      await deps.upsertObjectType(mapNoteTypeToArkObjectType(noteType));
      report.typeMigratedCount += 1;
    } catch (error) {
      report.typeFailedCount += 1;
      report.errors.push({
        id: noteType.id,
        phase: "object_type",
        message: (error as Error).message,
      });
    }
  }

  const existingObjectIds = new Set(arkObjects.map((object) => object.id));
  const normalizedEntries = heartEntries.map((entry) =>
    normalizeEntry({
      ...entry,
      type_id: entry.type_id ?? DEFAULT_ARK_TYPE_ID,
      header_layout: entry.header_layout ?? "default",
    }),
  );

  for (const normalizedEntry of normalizedEntries) {
    if (existingObjectIds.has(normalizedEntry.id)) {
      report.objectSkippedCount += 1;
      continue;
    }

    try {
      await deps.upsertObject(mapEntryToArkObject(normalizedEntry));
      existingObjectIds.add(normalizedEntry.id);
      report.objectMigratedCount += 1;
    } catch (error) {
      report.objectFailedCount += 1;
      report.errors.push({
        id: normalizedEntry.id,
        phase: "object",
        message: (error as Error).message,
      });
    }
  }

  for (const normalizedEntry of normalizedEntries) {
    await syncArkObjectLinks(deps, normalizedEntry, report);
  }

  report.finishedAt = now();
  report.status = buildReportStatus(report);
  return report;
}
