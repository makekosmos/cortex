import fs from "node:fs";

import path from "node:path";

import { app } from "electron";
import {
  buildPersonalSelectedSpace,
  readSharedSelectedSpace,
  writeSharedSelectedSpace,
} from "../../../../packages/shared-space/selectedSpace";

import { defaultCodeToolsSettings, type CodeToolsSettings } from "./codeTools";

import { runHeartRequest } from "./heart";

import {
  normalizeSlug,
  noteTypeSchema,
  parseHeaderTemplate,
  parseNoteTypeDefinition,
  parseNoteTypeUiSchema,
  validateHeaderProps,
  type NoteType,
} from "@/lib/typedNotes";
import { normalizeSystemNoteType } from "@/lib/systemTypes";
import {
  runArkRequest,
  shutdownArk,
  type ArkObjectLinkRecord,
  type ArkObjectRecord,
  type ArkObjectTypeRecord,
} from "./ark";
import { runHeartVaultToArkObjectMigration } from "./ark-object-migration";

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

export interface Folder {
  id: string;

  name: string;

  created_at: number;

  parent_id: string | null;
}

export interface SearchResult {
  file: string;

  line: number;

  text: string;

  entryId: string;
}

export type CreateFolderResult =
  | {
      ok: true;

      folder: Folder;
    }
  | {
      ok: false;

      reason: "duplicate_folder_name";

      name: string;
    };

export type MoveFolderResult =
  | {
      ok: true;

      folderId: string;

      parent_id: string | null;
    }
  | {
      ok: false;

      reason: "folder_not_found" | "invalid_target" | "duplicate_folder_name";

      message: string;
    };

export type SaveEntryResult =
  | {
      ok: true;

      entryId: string;
    }
  | {
      ok: false;

      reason: "duplicate_title";

      conflictingEntryId: string;

      title: string;

      folder_id: string | null;
    }
  | {
      ok: false;

      reason: "invalid_type_metadata";

      message: string;
    };

export type DeleteEntryResult =
  | {
      ok: true;

      entryId: string;
    }
  | {
      ok: false;

      reason: "entry_not_found";

      message: string;
    };

export type DeleteFolderResult =
  | {
      ok: true;

      folderId: string;
    }
  | {
      ok: false;

      reason: "folder_not_found";

      message: string;
    }
  | {
      ok: false;

      reason: "folder_not_empty";

      message: string;

      entryCount: number;
    };

export type SaveNoteTypeResult =
  | {
      ok: true;

      noteType: NoteType;
    }
  | {
      ok: false;

      reason: "duplicate_slug" | "invalid_definition";

      message: string;
    };

export interface ExportMarkdownVaultResult {
  ok: boolean;

  exportedCount: number;

  outputDir: string;
}

type MoveEntryToFolderInternalResult =
  | {
      ok: true;

      entryId: string;

      folder_id: string | null;
    }
  | {
      ok: false;

      reason: "duplicate_title" | "entry_not_found" | "folder_not_found";

      message: string;
    };

interface AppConfig {
  vaultPath?: string;

  recentVaultPaths?: string[];

  codeToolsSettings?: Partial<CodeToolsSettings>;

  sidebarWidth?: number;

  sidebarCollapsed?: boolean;

  widgetSidebarWidth?: number;

  widgetSidebarCollapsed?: boolean;

  hevyAuthToken?: string;

  hevyUsername?: string;
}

let currentVaultPath: string | null = null;

function getSharedSelectedSpace() {
  return readSharedSelectedSpace(app.getPath("appData"));
}

function getConfigPath() {
  return path.join(app.getPath("userData"), "config.json");
}

function readAppConfig(): AppConfig {
  const configPath = getConfigPath();

  if (!fs.existsSync(configPath)) {
    return {};
  }

  try {
    return JSON.parse(fs.readFileSync(configPath, "utf-8")) as AppConfig;
  } catch {
    return {};
  }
}

function writeAppConfig(config: AppConfig) {
  fs.writeFileSync(getConfigPath(), JSON.stringify(config, null, 2), "utf-8");
}

function normalizeRecentVaultPaths(
  paths: string[],
  activeVaultPath?: string | null,
) {
  const uniquePaths = Array.from(new Set(paths.filter(Boolean)));

  const orderedPaths = activeVaultPath
    ? [
        activeVaultPath,
        ...uniquePaths.filter((item) => item !== activeVaultPath),
      ]
    : uniquePaths;

  return orderedPaths.slice(0, 8);
}

function mergeCodeToolsSettings(
  partial?: Partial<CodeToolsSettings>,
): CodeToolsSettings {
  return {
    ...defaultCodeToolsSettings,

    ...partial,
  };
}

const DEFAULT_ARK_TYPE_ID = "note_obj";
const ARK_OBJECT_TYPE_IDS = new Set(["note_obj", "game_obj", "task_obj"]);

function arkTimestampToMillis(value?: string | null) {
  if (!value) {
    return Date.now();
  }
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : Date.now();
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

function stringifyHeaderProps(props: Record<string, unknown>) {
  return JSON.stringify(props);
}

function ensureArkList<T>(value: unknown): T[] {
  if (Array.isArray(value)) {
    return value as T[];
  }

  if (value && typeof value === "object") {
    const record = value as Record<string, unknown>;

    if (Array.isArray(record.items)) {
      return record.items as T[];
    }

    if (Array.isArray(record.objects)) {
      return record.objects as T[];
    }

    if (Array.isArray(record.links)) {
      return record.links as T[];
    }

    if (Array.isArray(record.types)) {
      return record.types as T[];
    }
  }

  return [];
}

function mapArkObjectToEntry(
  object: ArkObjectRecord,
  links: ArkObjectLinkRecord[],
): Entry {
  const headerProps = {
    ...object.propsJson,
    related_notes: links
      .filter((link) => link.sourceObjectId === object.id && link.linkType === "related")
      .map((link) => link.targetObjectId),
  };

  return normalizeEntry({
    id: object.id,
    title: object.title,
    content_json: JSON.stringify(object.contentJson ?? { type: "doc", content: [{ type: "paragraph" }] }),
    created_at: arkTimestampToMillis(object.createdAt),
    updated_at: arkTimestampToMillis(object.updatedAt),
    folder_id: null,
    type_id: object.typeId,
    header_layout: "default",
    header_props_json: stringifyHeaderProps(headerProps),
    schema_version: 1,
    deleted_at: object.deletedAt ? arkTimestampToMillis(object.deletedAt) : null,
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

function mapArkObjectTypeToNoteType(objectType: ArkObjectTypeRecord): NoteType {
  const uiSchema = parseNoteTypeUiSchema(objectType.uiSchemaJson);
  const headerTemplate = {
    kind: uiSchema.header_layout === "column" ? "centered_profile" : "default",
    primaryFieldIds: uiSchema.featured_fields ?? [],
    secondaryFieldIds: uiSchema.visible_fields ?? [],
    imageFieldId: null,
  };

  const icon =
    objectType.id === "game_obj"
      ? "game-controller"
      : objectType.id === "task_obj"
        ? "checkmark-circle"
        : "document-text";

  const color =
    objectType.id === "game_obj"
      ? "#ef4444"
      : objectType.id === "task_obj"
        ? "#f59e0b"
        : "#2aa7ee";

  return normalizeSystemNoteType(normalizeNoteType({
    id: objectType.id,
    name: objectType.name,
    slug: normalizeSlug(objectType.id),
    icon,
    color,
    schema_json: objectType.schemaJson,
    header_template_json: JSON.stringify(headerTemplate),
    ui_schema_json: objectType.uiSchemaJson,
    created_at: arkTimestampToMillis(objectType.createdAt),
    updated_at: arkTimestampToMillis(objectType.updatedAt),
  }));
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

async function listArkObjects(): Promise<Entry[]> {
  const [objects, links] = await Promise.all([
    runArkRequest<ArkObjectRecord[] | { items?: ArkObjectRecord[]; objects?: ArkObjectRecord[] }>({
      operation: "list_objects",
    }),
    runArkRequest<ArkObjectLinkRecord[] | { items?: ArkObjectLinkRecord[]; links?: ArkObjectLinkRecord[] }>({
      operation: "list_object_links",
    }),
  ]);

  const normalizedLinks = ensureArkList<ArkObjectLinkRecord>(links);

  return ensureArkList<ArkObjectRecord>(objects).map((object) => mapArkObjectToEntry(object, normalizedLinks));
}

async function migrateHeartVaultToArkObjects(vaultPath: string) {
  const report = await runHeartVaultToArkObjectMigration({
    listNoteTypes: (path) =>
      runHeartRequest<NoteType[]>({
        operation: "list_note_types",
        vaultPath: path,
      }),
    listEntries: (path) =>
      runHeartRequest<Entry[]>({
        operation: "list_entries",
        vaultPath: path,
      }),
    listObjects: () =>
      runArkRequest<ArkObjectRecord[] | { items?: ArkObjectRecord[]; objects?: ArkObjectRecord[] }>({
        operation: "list_objects",
      }).then((objects) => ensureArkList<ArkObjectRecord>(objects)),
    listObjectLinks: () =>
      runArkRequest<ArkObjectLinkRecord[] | { items?: ArkObjectLinkRecord[]; links?: ArkObjectLinkRecord[] }>({
        operation: "list_object_links",
      }).then((links) => ensureArkList<ArkObjectLinkRecord>(links)),
    upsertObjectType: (objectType) =>
      runArkRequest<boolean>({
        operation: "upsert_object_type",
        object_type: {
          id: objectType.id,
          name: objectType.name,
          schemaJson: objectType.schemaJson,
          uiSchemaJson: objectType.uiSchemaJson,
          createdAt: objectType.createdAt,
          updatedAt: objectType.updatedAt,
          systemLocked: objectType.systemLocked,
        },
      }),
    upsertObject: (object) =>
      runArkRequest<boolean>({
        operation: "upsert_object",
        object: {
          id: object.id,
          typeId: object.typeId,
          title: object.title,
          contentJson: object.contentJson,
          propsJson: object.propsJson,
          createdAt: object.createdAt,
          updatedAt: object.updatedAt,
          deletedAt: object.deletedAt ?? null,
        },
      }),
    upsertObjectLink: (objectLink) =>
      runArkRequest<boolean>({
        operation: "upsert_object_link",
        object_link: {
          id: objectLink.id,
          sourceObjectId: objectLink.sourceObjectId,
          targetObjectId: objectLink.targetObjectId,
          linkType: objectLink.linkType,
          createdAt: objectLink.createdAt,
        },
      }),
    deleteObjectLink: (id) => runArkRequest<boolean>({ operation: "delete_object_link", id }),
  }, vaultPath, {
    backupDir: path.join(app.getPath("userData"), ".migration-backups"),
  });

  if (report.status === "partial_failure" || report.status === "failed") {
    console.warn("[Eden] ARK note object migration completed with errors:", report);
  }
}

async function getArkEntry(id: string): Promise<Entry | undefined> {
  const [object, links] = await Promise.all([
    runArkRequest<ArkObjectRecord | null>({ operation: "get_object", id }),
    runArkRequest<ArkObjectLinkRecord[]>({ operation: "list_object_links" }),
  ]);

  return object ? mapArkObjectToEntry(object, links) : undefined;
}

async function syncArkObjectLinks(entry: Entry) {
  const headerProps = parseHeaderPropsJson(entry.header_props_json);
  const relatedNotes = Array.isArray(headerProps.related_notes)
    ? headerProps.related_notes.filter((value): value is string => typeof value === "string")
    : [];

  const existingLinks = await runArkRequest<ArkObjectLinkRecord[]>({
    operation: "list_object_links",
  });

  const ownedLinks = existingLinks.filter(
    (link) => link.sourceObjectId === entry.id && link.linkType === "related",
  );

  for (const link of ownedLinks) {
    if (!relatedNotes.includes(link.targetObjectId)) {
      await runArkRequest<boolean>({ operation: "delete_object_link", id: link.id });
    }
  }

  for (const targetObjectId of relatedNotes) {
    const existing = ownedLinks.find((link) => link.targetObjectId === targetObjectId);
    if (existing) {
      continue;
    }

    await runArkRequest<boolean>({
      operation: "upsert_object_link",
      object_link: {
        id: `${entry.id}:related:${targetObjectId}`,
        sourceObjectId: entry.id,
        targetObjectId: targetObjectId,
        linkType: "related",
        createdAt: millisToArkTimestamp(entry.updated_at),
      },
    });
  }
}

function requireVaultPath() {
  const vaultPath = currentVaultPath ?? getVaultPath();

  if (!vaultPath) {
    throw new Error("Vault path is not configured");
  }

  currentVaultPath = vaultPath;

  return vaultPath;
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

async function getNoteTypeById(noteTypeId: string) {
  if (ARK_OBJECT_TYPE_IDS.has(noteTypeId)) {
    const objectType = await runArkRequest<ArkObjectTypeRecord | null>({
      operation: "get_object_type",
      id: noteTypeId,
    });

    return objectType ? mapArkObjectTypeToNoteType(objectType) : null;
  }

  const vaultPath = requireVaultPath();

  return runHeartRequest<NoteType | null>({
    operation: "get_note_type_by_id",

    vaultPath,

    noteTypeId,
  });
}

async function validateEntryTypeMetadata(
  entry: Entry,
): Promise<SaveEntryResult | null> {
  if (!entry.type_id) {
    return null;
  }

  const noteType = await getNoteTypeById(entry.type_id);

  if (!noteType) {
    return {
      ok: false,

      reason: "invalid_type_metadata",

      message: "Тип заметки больше не существует",
    };
  }

  let parsedProps: unknown = {};

  try {
    parsedProps = JSON.parse(entry.header_props_json || "{}");
  } catch {
    return {
      ok: false,

      reason: "invalid_type_metadata",

      message: "Верхушка заметки сохранена в неверном формате",
    };
  }

  const validation = validateHeaderProps(noteType, parsedProps);

  if (!validation.success) {
    return {
      ok: false,

      reason: "invalid_type_metadata",

      message: "Структура верхушки заметки больше не соответствует типу",
    };
  }

  return null;
}

export function getVaultPath(): string | null {
  return readAppConfig().vaultPath || getSharedSelectedSpace()?.vaultPath || null;
}

export function getRecentVaultPaths(): string[] {
  const config = readAppConfig();
  const sharedVaultPath = getSharedSelectedSpace()?.vaultPath ?? null;

  return normalizeRecentVaultPaths(
    sharedVaultPath
      ? [sharedVaultPath, ...(config.recentVaultPaths ?? [])]
      : (config.recentVaultPaths ?? []),
    config.vaultPath ?? sharedVaultPath ?? null,
  );
}

export async function initStore(vaultPath?: string) {
  currentVaultPath = vaultPath || getVaultPath();

  if (!currentVaultPath) {
    return false;
  }

  await runHeartRequest<boolean>({
    operation: "init_store",

    vaultPath: currentVaultPath,
  });

  await migrateHeartVaultToArkObjects(currentVaultPath);

  return true;
}

export async function setVaultPath(newPath: string) {
  const config = readAppConfig();
  const sharedSelection = buildPersonalSelectedSpace(newPath, "eden");

  writeAppConfig({
    ...config,

    vaultPath: newPath,

    recentVaultPaths: normalizeRecentVaultPaths(
      [newPath, ...(config.recentVaultPaths ?? [])],

      newPath,
    ),
  });

  writeSharedSelectedSpace(app.getPath("appData"), sharedSelection);
  // The Ark RPC child keeps the DB path from its init request.
  // Reset it so the next Ark request re-initializes against the new selected space.
  shutdownArk();

  await initStore(newPath);
}

export function getCodeToolsSettings(): CodeToolsSettings {
  return mergeCodeToolsSettings(readAppConfig().codeToolsSettings);
}

export function updateCodeToolsSettings(
  settings: Partial<CodeToolsSettings>,
): CodeToolsSettings {
  const config = readAppConfig();

  const mergedSettings = mergeCodeToolsSettings({
    ...config.codeToolsSettings,

    ...settings,
  });

  writeAppConfig({
    ...config,

    codeToolsSettings: mergedSettings,
  });

  return mergedSettings;
}

export interface SidebarPanelConfig {
  width: number;

  collapsed: boolean;
}

export interface SidebarConfig {
  widget: SidebarPanelConfig;
}

export interface SidebarConfigPatch {
  widget?: Partial<SidebarPanelConfig>;
}

const DEFAULT_WIDGET_SIDEBAR_WIDTH = 320;

const MIN_WIDGET_SIDEBAR_WIDTH = 220;

const MAX_WIDGET_SIDEBAR_WIDTH = 520;

const COLLAPSE_THRESHOLD = 60;

export function getSidebarConfig(): SidebarConfig {
  const config = readAppConfig();

  return {
    widget: {
      width: Math.max(
        MIN_WIDGET_SIDEBAR_WIDTH,

        Math.min(
          MAX_WIDGET_SIDEBAR_WIDTH,

          config.widgetSidebarWidth ??
            config.sidebarWidth ??
            DEFAULT_WIDGET_SIDEBAR_WIDTH,
        ),
      ),

      collapsed:
        config.widgetSidebarCollapsed ?? config.sidebarCollapsed ?? false,
    },
  };
}

export function updateSidebarConfig(
  partial: SidebarConfigPatch,
): SidebarConfig {
  const current = getSidebarConfig();

  const updated: SidebarConfig = {
    widget: { ...current.widget, ...partial.widget },
  };

  const config = readAppConfig();

  writeAppConfig({
    ...config,

    widgetSidebarWidth: updated.widget.width,

    widgetSidebarCollapsed: updated.widget.collapsed,
  });

  return updated;
}

export {
  MIN_WIDGET_SIDEBAR_WIDTH,
  MAX_WIDGET_SIDEBAR_WIDTH,
  COLLAPSE_THRESHOLD,
};

export async function saveEntry(entry: Entry): Promise<SaveEntryResult> {
  const vaultPath = requireVaultPath();

  const normalizedEntry = normalizeEntry({
    ...entry,
    type_id: entry.type_id ?? DEFAULT_ARK_TYPE_ID,
    header_layout: entry.header_layout ?? "default",
  });

  const validationResult = await validateEntryTypeMetadata(normalizedEntry);

  if (validationResult) {
    return validationResult;
  }

  if (ARK_OBJECT_TYPE_IDS.has(normalizedEntry.type_id ?? DEFAULT_ARK_TYPE_ID)) {
    const arkObject = mapEntryToArkObject(normalizedEntry);
    await runArkRequest<boolean>({
      operation: "upsert_object",
      object: {
        id: arkObject.id,
        typeId: arkObject.typeId,
        title: arkObject.title,
        contentJson: arkObject.contentJson,
        propsJson: arkObject.propsJson,
        createdAt: arkObject.createdAt,
        updatedAt: arkObject.updatedAt,
        deletedAt: arkObject.deletedAt ?? null,
      },
    });
    await syncArkObjectLinks(normalizedEntry);

    return {
      ok: true,
      entryId: normalizedEntry.id,
    };
  }

  return runHeartRequest<SaveEntryResult>({
    operation: "save_entry",

    vaultPath,

    entry: normalizedEntry,
  });
}

export async function exportMarkdownVault(
  outputDir: string,
): Promise<ExportMarkdownVaultResult> {
  const vaultPath = requireVaultPath();

  return runHeartRequest<ExportMarkdownVaultResult>({
    operation: "export_markdown_vault",

    vaultPath,

    outputDir,
  });
}

export async function loadEntry(id: string): Promise<Entry | undefined> {
  const vaultPath = currentVaultPath ?? getVaultPath();

  const arkEntry = await getArkEntry(id);
  if (arkEntry) {
    return arkEntry;
  }

  if (!vaultPath) {
    return undefined;
  }

  const entry = await runHeartRequest<Entry | null>({
    operation: "load_entry",

    vaultPath,

    id,
  });

  return entry ? normalizeEntry(entry) : undefined;
}

export async function listEntries(): Promise<Entry[]> {
  const vaultPath = currentVaultPath ?? getVaultPath();

  const [arkEntries, heartEntries] = await Promise.all([
    listArkObjects(),
    vaultPath
      ? runHeartRequest<Entry[]>({
        operation: "list_entries",
        vaultPath,
      }).then((entries) => entries.map(normalizeEntry))
      : Promise.resolve([]),
  ]);

  const byId = new Map<string, Entry>();
  for (const entry of heartEntries) {
    byId.set(entry.id, entry);
  }
  for (const entry of arkEntries) {
    byId.set(entry.id, entry);
  }
  return [...byId.values()].sort((a, b) => b.updated_at - a.updated_at);
}

export async function searchEntries(query: string): Promise<SearchResult[]> {
  const vaultPath = currentVaultPath ?? getVaultPath();

  if (!vaultPath || !query.trim()) {
    return [];
  }

  const [heartResults, arkResults] = await Promise.all([
    runHeartRequest<SearchResult[]>({
      operation: "search_entries",
      vaultPath,
      query,
    }),
    runArkRequest<SearchResult[]>({
      operation: "search_objects",
      query,
    }),
  ]);

  const seen = new Set<string>();

  return [...arkResults, ...heartResults].filter((result) => {
    const key = `${result.entryId}:${result.line}:${result.text}`;
    if (seen.has(key)) {
      return false;
    }

    seen.add(key);
    return true;
  });
}

export async function createFolder(
  id: string,

  name: string,

  parentId: string | null = null,
): Promise<CreateFolderResult> {
  const vaultPath = requireVaultPath();

  return runHeartRequest<CreateFolderResult>({
    operation: "create_folder",

    vaultPath,

    id,

    name,

    parentId,
  });
}

export async function listFolders(): Promise<Folder[]> {
  const vaultPath = currentVaultPath ?? getVaultPath();

  if (!vaultPath) {
    return [];
  }

  return runHeartRequest<Folder[]>({
    operation: "list_folders",

    vaultPath,
  });
}

export async function listNoteTypes(): Promise<NoteType[]> {
  const vaultPath = currentVaultPath ?? getVaultPath();

  const [arkTypes, heartTypes] = await Promise.all([
    runArkRequest<ArkObjectTypeRecord[] | { items?: ArkObjectTypeRecord[]; types?: ArkObjectTypeRecord[] }>({
      operation: "list_object_types",
    }).then((types) =>
      ensureArkList<ArkObjectTypeRecord>(types).map(mapArkObjectTypeToNoteType)
    ),
    vaultPath
      ? runHeartRequest<NoteType[]>({
        operation: "list_note_types",
        vaultPath,
      }).then((types) => types.map(normalizeNoteType))
      : Promise.resolve([]),
  ]);

  const byId = new Map<string, NoteType>();
  for (const noteType of heartTypes) {
    byId.set(noteType.id, noteType);
  }
  for (const noteType of arkTypes) {
    byId.set(noteType.id, noteType);
  }
  return [...byId.values()];
}

export async function saveNoteType(
  noteType: NoteType,
): Promise<SaveNoteTypeResult> {
  const vaultPath = requireVaultPath();

  try {
    parseNoteTypeDefinition(noteType.schema_json);
    parseHeaderTemplate(noteType.header_template_json);
    parseNoteTypeUiSchema(noteType.ui_schema_json);
  } catch {
    return {
      ok: false,

      reason: "invalid_definition",

      message: "Схема типа заметки заполнена некорректно",
    };
  }

  const normalizedNoteType = normalizeNoteType({
    ...noteType,

    slug: normalizeSlug(noteType.slug || noteType.name),
  });

  const arkObjectType = mapNoteTypeToArkObjectType(normalizedNoteType);
  await runArkRequest<boolean>({
    operation: "upsert_object_type",
    object_type: {
      id: arkObjectType.id,
      name: arkObjectType.name,
      schemaJson: arkObjectType.schemaJson,
      uiSchemaJson: arkObjectType.uiSchemaJson,
      createdAt: arkObjectType.createdAt,
      updatedAt: arkObjectType.updatedAt,
      systemLocked: arkObjectType.systemLocked,
    },
  });

  if (ARK_OBJECT_TYPE_IDS.has(normalizedNoteType.id)) {
    return {
      ok: true,
      noteType: normalizedNoteType,
    };
  }

  return runHeartRequest<SaveNoteTypeResult>({
    operation: "save_note_type",
    vaultPath,
    note_type: normalizedNoteType,
  });
}

export async function deleteNoteType(noteTypeId: string) {
  const vaultPath = requireVaultPath();

  await runArkRequest<boolean>({
    operation: "delete_object_type",
    id: noteTypeId,
  });

  if (ARK_OBJECT_TYPE_IDS.has(noteTypeId)) {
    return true;
  }

  return runHeartRequest<boolean>({
    operation: "delete_note_type",

    vaultPath,

    noteTypeId,
  });
}

export async function moveEntryToFolder(
  entryId: string,
  folderId: string | null,
) {
  const vaultPath = requireVaultPath();

  const result = await runHeartRequest<MoveEntryToFolderInternalResult>({
    operation: "move_entry_to_folder",

    vaultPath,

    entryId,

    folderId,
  });

  if (!result.ok) {
    throw new Error(result.message);
  }

  return true;
}

export async function deleteEntry(entryId: string): Promise<DeleteEntryResult> {
  const arkEntry = await getArkEntry(entryId);
  if (arkEntry) {
    await runArkRequest<boolean>({
      operation: "delete_object",
      id: entryId,
    });

    return {
      ok: true,
      entryId,
    };
  }

  const vaultPath = requireVaultPath();

  return runHeartRequest<DeleteEntryResult>({
    operation: "delete_entry",

    vaultPath,

    entryId,
  });
}

export async function deleteFolder(
  folderId: string,
): Promise<DeleteFolderResult> {
  const vaultPath = requireVaultPath();

  return runHeartRequest<DeleteFolderResult>({
    operation: "delete_folder",

    vaultPath,

    folderId,
  });
}

export async function moveFolderToFolder(
  folderId: string,

  parentId: string | null,
): Promise<MoveFolderResult> {
  const vaultPath = requireVaultPath();

  return runHeartRequest<MoveFolderResult>({
    operation: "move_folder_to_folder",

    vaultPath,

    folderId,

    parentId,
  });
}

export interface VaultStorageInfo {
  textBytes: number;

  trashBytes: number;

  dbBytes: number;

  vaultBytes: number;

  entryCount: number;

  trashCount: number;
}

export async function listTrashEntries(): Promise<Entry[]> {
  const vaultPath = currentVaultPath ?? getVaultPath();

  if (!vaultPath) return [];

  const entries = await runHeartRequest<Entry[]>({
    operation: "list_trash_entries",

    vaultPath,
  });

  return entries.map(normalizeEntry);
}

export async function restoreEntry(entryId: string) {
  const vaultPath = requireVaultPath();

  return runHeartRequest<{ ok: boolean; entryId?: string }>({
    operation: "restore_entry",

    vaultPath,

    entryId,
  });
}

export async function permanentDeleteEntry(entryId: string) {
  const vaultPath = requireVaultPath();

  return runHeartRequest<{ ok: boolean; entryId?: string }>({
    operation: "permanent_delete_entry",

    vaultPath,

    entryId,
  });
}

export async function purgeExpiredTrash() {
  const vaultPath = requireVaultPath();

  const thirtyDaysMs = 30 * 24 * 60 * 60 * 1000;

  return runHeartRequest<{ ok: boolean; purgedCount: number }>({
    operation: "purge_expired_trash",

    vaultPath,

    maxAgeMs: thirtyDaysMs,
  });
}

export async function getVaultStorageInfo(): Promise<VaultStorageInfo> {
  const vaultPath = requireVaultPath();

  return runHeartRequest<VaultStorageInfo>({
    operation: "get_vault_storage_info",

    vaultPath,
  });
}

export function getHevyAuthToken(): string | null {
  return readAppConfig().hevyAuthToken || null;
}

export function getHevyUsername(): string | null {
  return readAppConfig().hevyUsername || null;
}

export function setHevyAuth(token: string, username: string) {
  const config = readAppConfig();

  writeAppConfig({ ...config, hevyAuthToken: token, hevyUsername: username });
}

export function clearHevyAuth() {
  const config = readAppConfig();

  const { hevyAuthToken: _, hevyUsername: __, ...rest } = config;

  writeAppConfig(rest);
}
