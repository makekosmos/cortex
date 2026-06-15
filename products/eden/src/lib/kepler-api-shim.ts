// kepler-api-shim — эмуляция `window.api` поверх `window.kepler.ark.request`.
//
// Eden как Kepler extension не имеет собственного Electron main процесса:
// все ARK операции идут через main proxy в kepler-shell. Этот модуль
// устанавливает `window.api` с такой же сигнатурой как в standalone Eden
// (см. `apps/eden/ts/main/preload.ts`), чтобы существующие store / components
// продолжали работать без массового rewrite call-sites.
//
// Phase 6.0 (2026-05-17): note CRUD + folders + note types + search через ARK.
// Phase 6.0.A (2026-05-17): code tools / vault picker / export — удалены.
// Trash работает поверх ARK soft-delete (deletedAt != null).

import { normalizeStatus, type TaskStatus } from "./taskStatus";
import {
  normalizeSlug,
  noteTypeSchema,
  parseHeaderTemplate,
  parseNoteTypeDefinition,
  parseNoteTypeUiSchema,
  validateHeaderProps,
  getNoteTypeCollectionName,
  type NoteType,
} from "@/lib/typedNotes";
import { writeEntryMarkdown } from "@/editor-cm/content";
import {
  SYSTEM_TYPE_COLLECTION,
  SYSTEM_TYPE_COLLECTION_ID,
  isSystemType,
  normalizeSystemNoteType,
  shouldShowAsEdenCollection,
} from "@/lib/systemTypes";

const DEFAULT_ARK_TYPE_ID = "note_obj";

interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string;
  contentJson: unknown;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
}

interface ArkObjectSummaryRecord {
  id: string;
  typeId: string;
  title: string;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
}

interface ArkObjectTypeRecord {
  id: string;
  name: string;
  schemaJson: string;
  uiSchemaJson: string;
  createdAt: string;
  updatedAt: string;
  systemLocked: boolean;
}

interface ArkObjectLinkRecord {
  id: string;
  sourceObjectId: string;
  targetObjectId: string;
  linkType: string;
  createdAt: string;
}

interface KeplerArkBridge {
  request: <T = unknown>(operation: string, params?: Record<string, unknown>) => Promise<T>;
  subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
}

interface KeplerWindowApi {
  close: () => void;
  minimize: () => void;
  maximize: () => void;
  zoomGet?: () => Promise<number>;
  zoomSet?: (factor: number) => Promise<number>;
}

interface KeplerNamespace {
  ark: KeplerArkBridge;
  window?: KeplerWindowApi;
}

function keplerBridge(): KeplerArkBridge {
  const k = (window as unknown as { kepler?: KeplerNamespace }).kepler;
  if (!k?.ark) {
    throw new Error("Eden extension: window.kepler.ark недоступен — preload не подключён");
  }
  return k.ark;
}

function keplerWindow(): KeplerWindowApi | null {
  const k = (window as unknown as { kepler?: KeplerNamespace }).kepler;
  return k?.window ?? null;
}

function ark<T = unknown>(operation: string, params?: Record<string, unknown>): Promise<T> {
  return keplerBridge().request<T>(operation, params);
}

function ensureList<T>(value: unknown): T[] {
  if (Array.isArray(value)) return value as T[];
  if (value && typeof value === "object") {
    const r = value as Record<string, unknown>;
    if (Array.isArray(r.items)) return r.items as T[];
    if (Array.isArray(r.objects)) return r.objects as T[];
    if (Array.isArray(r.links)) return r.links as T[];
    if (Array.isArray(r.types)) return r.types as T[];
  }
  return [];
}

function arkTimestampToMillis(value: string | null | undefined, fallback: number): number {
  if (!value) return fallback;
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function millisToArkTimestamp(value: number | null | undefined): string {
  const ts = typeof value === "number" ? value : Date.now();
  return new Date(ts).toISOString();
}

function parseHeaderPropsJson(raw: string | null | undefined): Record<string, unknown> {
  if (!raw?.trim()) return {};
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function stringifyHeaderProps(props: Record<string, unknown>): string {
  return JSON.stringify(props);
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

function normalizeNoteType(noteType: NoteType): NoteType {
  return noteTypeSchema.parse({
    ...noteType,
    ui_schema_json: noteType.ui_schema_json ?? JSON.stringify({}),
  });
}

function mapArkObjectToEntry(
  object: ArkObjectRecord,
  links: ArkObjectLinkRecord[],
  objectType?: ArkObjectTypeRecord,
): Entry {
  const createdAt = arkTimestampToMillis(object.createdAt, 0);
  const updatedAt = arkTimestampToMillis(object.updatedAt, createdAt);
  const headerProps = {
    ...object.propsJson,
    related_notes: links
      .filter((l) => l.sourceObjectId === object.id && l.linkType === "related")
      .map((l) => l.targetObjectId),
  };

  return normalizeEntry({
    id: object.id,
    title: object.title,
    content_json: JSON.stringify(object.contentJson ?? writeEntryMarkdown("")),
    created_at: createdAt,
    updated_at: updatedAt,
    folder_id: null,
    type_id: object.typeId,
    header_layout: objectType
      ? (parseNoteTypeUiSchema(objectType.uiSchemaJson).header_layout ?? "default")
      : "default",
    header_props_json: stringifyHeaderProps(headerProps),
    schema_version: 1,
    deleted_at: object.deletedAt ? arkTimestampToMillis(object.deletedAt, updatedAt) : null,
  });
}

function mapArkObjectSummaryToEntry(
  object: ArkObjectSummaryRecord,
  links: ArkObjectLinkRecord[],
  objectType?: ArkObjectTypeRecord,
): Entry {
  const createdAt = arkTimestampToMillis(object.createdAt, 0);
  const updatedAt = arkTimestampToMillis(object.updatedAt, createdAt);
  const headerProps = {
    ...object.propsJson,
    related_notes: links
      .filter((l) => l.sourceObjectId === object.id && l.linkType === "related")
      .map((l) => l.targetObjectId),
  };

  return normalizeEntry({
    id: object.id,
    title: object.title,
    content_json: JSON.stringify(writeEntryMarkdown("")),
    created_at: createdAt,
    updated_at: updatedAt,
    folder_id: null,
    type_id: object.typeId,
    header_layout: objectType
      ? (parseNoteTypeUiSchema(objectType.uiSchemaJson).header_layout ?? "default")
      : "default",
    header_props_json: stringifyHeaderProps(headerProps),
    schema_version: 1,
    deleted_at: object.deletedAt ? arkTimestampToMillis(object.deletedAt, updatedAt) : null,
  });
}

function mapEntryToArkObject(entry: Entry): ArkObjectRecord {
  const headerProps = parseHeaderPropsJson(entry.header_props_json);
  const { related_notes: _ignored, ...propsJson } = headerProps;
  let contentJson: unknown = writeEntryMarkdown("");
  try {
    contentJson = JSON.parse(entry.content_json || JSON.stringify(contentJson));
  } catch {
    // keep default
  }

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
  const createdAt = arkTimestampToMillis(objectType.createdAt, 0);
  const updatedAt = arkTimestampToMillis(objectType.updatedAt, createdAt);
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
    objectType.id === "game_obj" ? "#ef4444" : objectType.id === "task_obj" ? "#f59e0b" : "#2aa7ee";

  return normalizeSystemNoteType(
    normalizeNoteType({
      id: objectType.id,
      name: objectType.name,
      slug: normalizeSlug(objectType.id),
      icon,
      color,
      schema_json: objectType.schemaJson,
      header_template_json: JSON.stringify(headerTemplate),
      ui_schema_json: objectType.uiSchemaJson,
      created_at: createdAt,
      updated_at: updatedAt,
    }),
  );
}

function mapNoteTypeToArkObjectType(noteType: NoteType): ArkObjectTypeRecord {
  const uiSchema = parseNoteTypeUiSchema(noteType.ui_schema_json);
  const definition = parseNoteTypeDefinition(noteType.schema_json);
  const featuredFromUi = uiSchema.featured_fields ?? [];
  const visibleFromFields = definition.fields.filter((f) => f.visible !== false).map((f) => f.id);
  const readOnlyFromFields = definition.fields.filter((f) => f.read_only === true).map((f) => f.id);

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
      field_order: uiSchema.field_order?.length
        ? uiSchema.field_order
        : definition.fields.map((f) => f.id),
      header_layout: uiSchema.header_layout ?? "inline",
      default_layout: uiSchema.default_layout ?? "page",
      default_template_id: uiSchema.default_template_id ?? null,
      collection_name: uiSchema.collection_name,
    }),
    createdAt: millisToArkTimestamp(noteType.created_at),
    updatedAt: millisToArkTimestamp(noteType.updated_at),
    systemLocked: isSystemType(noteType.id),
  };
}

function collectionObjectIdForType(noteTypeId: string): string {
  return `collection:${noteTypeId}`;
}

function mapNoteTypeToCollectionEntry(
  noteType: NoteType,
  existing?: ArkObjectRecord | null,
): Entry {
  const now = Date.now();
  const createdAt = existing ? arkTimestampToMillis(existing.createdAt, now) : now;
  const updatedAt = existing ? arkTimestampToMillis(existing.updatedAt, createdAt) : now;

  return normalizeEntry({
    id: collectionObjectIdForType(noteType.id),
    title: getNoteTypeCollectionName(noteType),
    content_json: JSON.stringify(writeEntryMarkdown("")),
    created_at: createdAt,
    updated_at: updatedAt,
    folder_id: null,
    type_id: SYSTEM_TYPE_COLLECTION_ID,
    header_layout:
      parseNoteTypeUiSchema(SYSTEM_TYPE_COLLECTION.ui_schema_json).header_layout ?? "inline",
    header_props_json: stringifyHeaderProps({
      ...existing?.propsJson,
      object_type_id: noteType.id,
    }),
    schema_version: 1,
    deleted_at: existing?.deletedAt ? arkTimestampToMillis(existing.deletedAt, updatedAt) : null,
  });
}

function shouldIncludeObjectInEdenList(object: {
  typeId: string;
  propsJson?: Record<string, unknown>;
}): boolean {
  if (object.typeId !== SYSTEM_TYPE_COLLECTION_ID) return true;
  const objectTypeId = object.propsJson?.object_type_id;
  return typeof objectTypeId === "string" && shouldShowAsEdenCollection(objectTypeId);
}

// ---------------------------------------------------------------------------
// ARK-backed operations
// ---------------------------------------------------------------------------

export async function listEntries(): Promise<Entry[]> {
  const visibleTypeIds = readVisibleObjectTypeIds();
  const objects = await listObjectSummariesForVisibleTypes(visibleTypeIds);

  return objects
    .filter((o) => !o.deletedAt)
    .filter(shouldIncludeObjectInEdenList)
    .map((o) => mapArkObjectSummaryToEntry(o, [], undefined))
    .sort((a, b) => b.updated_at - a.updated_at);
}

async function listObjectSummariesForVisibleTypes(
  typeIds: string[],
): Promise<ArkObjectSummaryRecord[]> {
  try {
    if (typeIds.length === 0) {
      return await ark<unknown>("list_object_summaries").then(ensureList<ArkObjectSummaryRecord>);
    }

    const chunks = await Promise.all(
      typeIds.map((typeId) =>
        ark<unknown>("list_object_summaries_by_type", { type_id: typeId }).then(
          ensureList<ArkObjectSummaryRecord>,
        ),
      ),
    );
    return chunks.flat();
  } catch (err) {
    console.warn("[eden-extension] list_object_summaries unavailable, using full objects:", err);
    const fullObjects = await listObjectsForVisibleTypes(typeIds);
    return fullObjects.map((object) => ({
      id: object.id,
      typeId: object.typeId,
      title: object.title,
      propsJson: object.propsJson,
      createdAt: object.createdAt,
      updatedAt: object.updatedAt,
      deletedAt: object.deletedAt ?? null,
    }));
  }
}

async function listObjectsForVisibleTypes(typeIds: string[]): Promise<ArkObjectRecord[]> {
  if (typeIds.length === 0) {
    return ark<unknown>("list_objects").then(ensureList<ArkObjectRecord>);
  }

  const chunks = await Promise.all(
    typeIds.map((typeId) =>
      ark<unknown>("list_objects_by_type", { type_id: typeId }).then(ensureList<ArkObjectRecord>),
    ),
  );
  return chunks.flat();
}

export async function loadEntry(id: string): Promise<Entry | undefined> {
  const [object, links] = await Promise.all([
    ark<ArkObjectRecord | null>("get_object", { id }),
    ark<unknown>("list_object_links").then(ensureList<ArkObjectLinkRecord>),
  ]);

  if (!object || object.deletedAt) return undefined;
  const objectType = await ark<ArkObjectTypeRecord | null>("get_object_type", {
    id: object.typeId,
  }).catch(() => null);
  return mapArkObjectToEntry(object, links, objectType ?? undefined);
}

async function syncRelatedLinks(entry: Entry): Promise<void> {
  const headerProps = parseHeaderPropsJson(entry.header_props_json);
  const relatedNotes = Array.isArray(headerProps.related_notes)
    ? (headerProps.related_notes as unknown[]).filter((v): v is string => typeof v === "string")
    : [];

  const existingLinks = await ark<unknown>("list_object_links").then(
    ensureList<ArkObjectLinkRecord>,
  );

  const ownedLinks = existingLinks.filter(
    (l) => l.sourceObjectId === entry.id && l.linkType === "related",
  );

  for (const link of ownedLinks) {
    if (!relatedNotes.includes(link.targetObjectId)) {
      await ark<boolean>("delete_object_link", { id: link.id });
    }
  }

  for (const targetObjectId of relatedNotes) {
    if (ownedLinks.some((l) => l.targetObjectId === targetObjectId)) continue;
    await ark<boolean>("upsert_object_link", {
      object_link: {
        id: `${entry.id}:related:${targetObjectId}`,
        sourceObjectId: entry.id,
        targetObjectId,
        linkType: "related",
        createdAt: millisToArkTimestamp(entry.updated_at),
      },
    });
  }
}

async function getNoteTypeById(noteTypeId: string): Promise<NoteType | null> {
  const objectType = await ark<ArkObjectTypeRecord | null>("get_object_type", {
    id: noteTypeId,
  });
  return objectType ? mapArkObjectTypeToNoteType(objectType) : null;
}

async function validateEntryTypeMetadata(entry: Entry): Promise<SaveEntryResult | null> {
  if (!entry.type_id) return null;

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

async function ensureEntryTypeAvailable(noteTypeId: string): Promise<boolean> {
  const t = await ark<ArkObjectTypeRecord | null>("get_object_type", { id: noteTypeId });
  return Boolean(t);
}

export async function saveEntry(entry: Entry): Promise<SaveEntryResult> {
  const normalized = normalizeEntry({
    ...entry,
    type_id: entry.type_id ?? DEFAULT_ARK_TYPE_ID,
    header_layout: entry.header_layout ?? "default",
  });

  const validation = await validateEntryTypeMetadata(normalized);
  if (validation) return validation;

  const typeId = normalized.type_id ?? DEFAULT_ARK_TYPE_ID;
  if (!(await ensureEntryTypeAvailable(typeId))) {
    return {
      ok: false,
      reason: "invalid_type_metadata",
      message: "Тип заметки больше не существует",
    };
  }

  const normalizedTitle = normalized.title.trim().toLocaleLowerCase("ru");
  const existingObjects = await listAllObjects();
  const conflicting = existingObjects.find((candidate) => {
    if (candidate.id === normalized.id || candidate.deletedAt) return false;
    return candidate.title.trim().toLocaleLowerCase("ru") === normalizedTitle;
  });

  if (conflicting && normalizedTitle.length > 0) {
    return {
      ok: false,
      reason: "duplicate_title",
      conflictingEntryId: conflicting.id,
      title: normalized.title,
      folder_id: normalized.folder_id,
    };
  }

  const arkObject = mapEntryToArkObject(normalized);
  await ark<boolean>("upsert_object", {
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
  await syncRelatedLinks(normalized);

  return { ok: true, entryId: normalized.id };
}

export async function deleteEntry(entryId: string): Promise<DeleteEntryResult> {
  const existing = await ark<ArkObjectRecord | null>("get_object", { id: entryId });
  if (!existing || existing.deletedAt) {
    return {
      ok: false,
      reason: "entry_not_found",
      message: "Заметка не найдена в ARK",
    };
  }
  const deletedAt = millisToArkTimestamp(Date.now());
  await ark<boolean>("upsert_object", {
    object: {
      ...existing,
      updatedAt: deletedAt,
      deletedAt,
    },
  });
  return { ok: true, entryId };
}

export async function listNoteTypes(): Promise<NoteType[]> {
  const types = await ark<unknown>("list_object_types").then(ensureList<ArkObjectTypeRecord>);
  return types.map(mapArkObjectTypeToNoteType);
}

export async function ensureCollectionObjects(noteTypes: NoteType[]): Promise<Entry[]> {
  const collectionType = mapNoteTypeToArkObjectType(SYSTEM_TYPE_COLLECTION);
  await ark<boolean>("upsert_object_type", {
    object_type: {
      id: collectionType.id,
      name: collectionType.name,
      schemaJson: collectionType.schemaJson,
      uiSchemaJson: collectionType.uiSchemaJson,
      createdAt: collectionType.createdAt,
      updatedAt: collectionType.updatedAt,
      systemLocked: collectionType.systemLocked,
    },
  });

  const entries: Entry[] = [];
  for (const noteType of noteTypes) {
    if (!shouldShowAsEdenCollection(noteType.id)) continue;

    const existing = await ark<ArkObjectRecord | null>("get_object", {
      id: collectionObjectIdForType(noteType.id),
    }).catch(() => null);
    const entry = mapNoteTypeToCollectionEntry(noteType, existing);
    const arkObject = mapEntryToArkObject({
      ...entry,
      deleted_at: null,
    });

    await ark<boolean>("upsert_object", {
      object: {
        id: arkObject.id,
        typeId: arkObject.typeId,
        title: arkObject.title,
        contentJson: arkObject.contentJson,
        propsJson: arkObject.propsJson,
        createdAt: arkObject.createdAt,
        updatedAt: arkObject.updatedAt,
        deletedAt: null,
      },
    });
    entries.push({ ...entry, deleted_at: null });
  }

  return entries;
}

export async function saveNoteType(noteType: NoteType): Promise<SaveNoteTypeResult> {
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

  const normalized = normalizeNoteType({
    ...noteType,
    slug: normalizeSlug(noteType.slug || noteType.name),
  });

  const objectType = mapNoteTypeToArkObjectType(normalized);
  await ark<boolean>("upsert_object_type", {
    object_type: {
      id: objectType.id,
      name: objectType.name,
      schemaJson: objectType.schemaJson,
      uiSchemaJson: objectType.uiSchemaJson,
      createdAt: objectType.createdAt,
      updatedAt: objectType.updatedAt,
      systemLocked: objectType.systemLocked,
    },
  });

  return { ok: true, noteType: normalized };
}

export async function deleteNoteType(noteTypeId: string): Promise<boolean> {
  await ark<boolean>("delete_object_type", { id: noteTypeId });
  return true;
}

export async function searchEntries(query: string): Promise<SearchResult[]> {
  if (!query.trim()) return [];
  try {
    const results = await ark<SearchResult[]>("search_objects", { query });
    if (!Array.isArray(results)) return [];
    const seen = new Set<string>();
    return results.filter((r) => {
      const key = `${r.entryId}:${r.line}:${r.text}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  } catch (e) {
    console.warn("[eden-extension] search_objects failed:", e);
    return [];
  }
}

// ---------------------------------------------------------------------------
// Folder operations — Heart-dependent, stub в Phase 6.0.
// ---------------------------------------------------------------------------

export async function listFolders(): Promise<Folder[]> {
  return [];
}

export async function createFolder(
  _id: string,
  _name: string,
  _parentId: string | null = null,
): Promise<CreateFolderResult> {
  return {
    ok: false,
    reason: "duplicate_folder_name",
    name: "Папки недоступны в extension'е (Phase 6.0)",
  };
}

export async function moveEntryToFolder(
  _entryId: string,
  _folderId: string | null,
): Promise<boolean> {
  return false;
}

export async function moveFolderToFolder(
  _folderId: string,
  _parentId: string | null,
): Promise<MoveFolderResult> {
  return {
    ok: false,
    reason: "folder_not_found",
    message: "Папки недоступны в extension'е (Phase 6.0)",
  };
}

export async function deleteFolder(_folderId: string): Promise<DeleteFolderResult> {
  return {
    ok: false,
    reason: "folder_not_found",
    message: "Папки недоступны в extension'е (Phase 6.0)",
  };
}

// ---------------------------------------------------------------------------
// Vault / settings / persistent UI state — adapted к single-DB-per-user.
// ---------------------------------------------------------------------------

const VAULT_PATH_PLACEHOLDER = "kepler://ark";
const SIDEBAR_STORAGE_KEY = "eden-extension-sidebar-config";
const VISIBLE_OBJECT_TYPE_IDS_STORAGE_KEY = "eden-extension-visible-object-type-ids";

export async function getVaultPath(): Promise<string | null> {
  return VAULT_PATH_PLACEHOLDER;
}

async function getRecentVaultPaths(): Promise<string[]> {
  return [VAULT_PATH_PLACEHOLDER];
}

export async function setVaultPath(_path: string): Promise<boolean> {
  // Single ARK DB per user — no-op в extension'е.
  return true;
}

export async function selectFolder(): Promise<string | null> {
  // Dialogs недоступны без preload extension; UI button скрыт.
  return null;
}

async function openMarkdownVault(): Promise<MarkdownVaultOpenResult | null> {
  return window.kepler?.markdownFiles?.openVault?.() ?? null;
}

async function saveMarkdownVault(
  files: MarkdownVaultExportFile[],
): Promise<MarkdownVaultExportResult | null> {
  return window.kepler?.markdownFiles?.exportVault?.(files) ?? null;
}

async function openMarkdownFile(): Promise<MarkdownFileOpenResult | null> {
  return window.kepler?.markdownFiles?.open?.() ?? null;
}

async function saveMarkdownFile(
  suggestedName: string,
  content: string,
): Promise<MarkdownFileSaveResult | null> {
  return window.kepler?.markdownFiles?.save?.(suggestedName, content) ?? null;
}

// Renderer-types (`vite-env.d.ts`) declare `widget: { width, hidden }`,
// исторически main process сериализовался как `collapsed` — здесь следуем
// renderer-контракту (`hidden`).
interface SidebarConfig {
  widget: { width: number; hidden: boolean };
}

function readSidebarConfig(): SidebarConfig {
  try {
    const raw = localStorage.getItem(SIDEBAR_STORAGE_KEY);
    if (!raw) return { widget: { width: 320, hidden: false } };
    const parsed = JSON.parse(raw);
    return {
      widget: {
        width: Math.max(220, Math.min(520, Number(parsed?.widget?.width) || 320)),
        hidden: Boolean(parsed?.widget?.hidden ?? parsed?.widget?.collapsed ?? false),
      },
    };
  } catch {
    return { widget: { width: 320, hidden: false } };
  }
}

async function getSidebarConfig(): Promise<SidebarConfig> {
  return readSidebarConfig();
}

async function updateSidebarConfig(patch: {
  widget?: { width?: number; hidden?: boolean };
}): Promise<SidebarConfig> {
  const current = readSidebarConfig();
  const next: SidebarConfig = {
    widget: {
      width: patch.widget?.width ?? current.widget.width,
      hidden: patch.widget?.hidden ?? current.widget.hidden,
    },
  };
  try {
    localStorage.setItem(SIDEBAR_STORAGE_KEY, JSON.stringify(next));
  } catch {
    // ignore storage failures
  }
  return next;
}

function readVisibleObjectTypeIds(): string[] {
  try {
    const raw = localStorage.getItem(VISIBLE_OBJECT_TYPE_IDS_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];

    const seen = new Set<string>();
    return parsed
      .filter((value): value is string => typeof value === "string")
      .map((value) => value.trim())
      .filter(Boolean)
      .filter((value) => {
        if (seen.has(value)) return false;
        seen.add(value);
        return true;
      });
  } catch {
    return [];
  }
}

async function getEdenVisibleObjectTypeIds(): Promise<string[]> {
  return readVisibleObjectTypeIds();
}

async function setEdenVisibleObjectTypeIds(typeIds: string[]): Promise<string[]> {
  const seen = new Set<string>();
  const next = typeIds
    .filter((value): value is string => typeof value === "string")
    .map((value) => value.trim())
    .filter(Boolean)
    .filter((value) => {
      if (seen.has(value)) return false;
      seen.add(value);
      return true;
    });

  try {
    localStorage.setItem(VISIBLE_OBJECT_TYPE_IDS_STORAGE_KEY, JSON.stringify(next));
  } catch {
    // ignore storage failures
  }
  return next;
}

// ---------------------------------------------------------------------------
// Trash / storage — поверх ARK soft-delete (object'ы с `deletedAt != null`).
// ---------------------------------------------------------------------------

async function listAllObjects(): Promise<ArkObjectRecord[]> {
  return ark<unknown>("list_objects").then(ensureList<ArkObjectRecord>);
}

async function listTrashEntries(): Promise<Entry[]> {
  const [objects, links, objectTypes] = await Promise.all([
    listAllObjects(),
    ark<unknown>("list_object_links").then(ensureList<ArkObjectLinkRecord>),
    ark<unknown>("list_object_types").then(ensureList<ArkObjectTypeRecord>),
  ]);
  const typesById = new Map(objectTypes.map((t) => [t.id, t]));
  return objects
    .filter((o) => !!o.deletedAt)
    .map((o) => mapArkObjectToEntry(o, links, typesById.get(o.typeId)))
    .sort((a, b) => (b.deleted_at ?? 0) - (a.deleted_at ?? 0));
}

async function restoreEntry(entryId: string): Promise<{ ok: boolean; entryId?: string }> {
  const existing = await ark<ArkObjectRecord | null>("get_object", { id: entryId });
  if (!existing) return { ok: false, entryId };
  await ark<boolean>("upsert_object", {
    object: {
      id: existing.id,
      typeId: existing.typeId,
      title: existing.title,
      contentJson: existing.contentJson,
      propsJson: existing.propsJson,
      createdAt: existing.createdAt,
      updatedAt: millisToArkTimestamp(Date.now()),
      deletedAt: null,
    },
  });
  return { ok: true, entryId };
}

async function permanentDeleteEntry(entryId: string): Promise<{ ok: boolean; entryId?: string }> {
  // Hard-delete is only exposed from Trash UI after explicit confirmation.
  // Normal deleteEntry above is a soft delete via upsert_object(deletedAt).
  await ark<boolean>("delete_object", { id: entryId });
  return { ok: true, entryId };
}

async function purgeExpiredTrash(): Promise<{ ok: boolean; purgedCount: number }> {
  return { ok: true, purgedCount: 0 };
}

async function getVaultStorageInfo(): Promise<VaultStorageInfo> {
  const objects = await listAllObjects();
  const active = objects.filter((o) => !o.deletedAt);
  const trashed = objects.filter((o) => !!o.deletedAt);
  const sizeOf = (o: ArkObjectRecord) =>
    JSON.stringify(o.contentJson ?? "").length + (o.title?.length ?? 0);
  const textBytes = active.reduce((sum, o) => sum + sizeOf(o), 0);
  const trashBytes = trashed.reduce((sum, o) => sum + sizeOf(o), 0);
  return {
    textBytes,
    trashBytes,
    dbBytes: textBytes + trashBytes,
    vaultBytes: textBytes + trashBytes,
    entryCount: active.length,
    trashCount: trashed.length,
  };
}

async function getDiskFreeSpace(): Promise<number> {
  return 0;
}

// ---------------------------------------------------------------------------
// Platform / zoom / window controls.
// ---------------------------------------------------------------------------

async function getPlatform(): Promise<NodeJS.Platform> {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("windows")) return "win32";
  if (ua.includes("mac")) return "darwin";
  return "linux";
}

const ZOOM_STORAGE_KEY = "eden-extension-zoom";

async function zoomGet(): Promise<number> {
  const nativeZoomGet = keplerWindow()?.zoomGet;
  if (nativeZoomGet) {
    try {
      return await nativeZoomGet();
    } catch {
      // Fall back to browser/localStorage zoom below.
    }
  }

  const raw = localStorage.getItem(ZOOM_STORAGE_KEY);
  const parsed = raw ? parseFloat(raw) : NaN;
  return Number.isFinite(parsed) ? parsed : 1;
}

async function zoomSet(factor: number): Promise<number> {
  const clamped = Math.max(0.5, Math.min(2.0, factor));
  const nativeZoomSet = keplerWindow()?.zoomSet;

  if (nativeZoomSet) {
    const applied = await nativeZoomSet(clamped);
    try {
      localStorage.setItem(ZOOM_STORAGE_KEY, String(applied));
    } catch {
      // ignore
    }
    // Старые сборки масштабировали <html> через CSS zoom. Если пользователь
    // обновился без перезапуска renderer'а, убираем этот локальный shrink-layer:
    // нативный webContents zoom должен масштабировать viewport целиком.
    document.documentElement.style.zoom = "";
    return applied;
  }

  try {
    localStorage.setItem(ZOOM_STORAGE_KEY, String(clamped));
  } catch {
    // ignore
  }
  document.documentElement.style.zoom = String(clamped);
  return clamped;
}

function minimize(): void {
  keplerWindow()?.minimize();
}

function maximize(): void {
  keplerWindow()?.maximize();
}

function close(): void {
  keplerWindow()?.close();
}

// ---------------------------------------------------------------------------
// Command dispatch — Eden слушает eden:cmd:note:create / eden:cmd:note:search
// через `window.api.onCommand(channel, handler)`. В extension'е этот канал
// диспатчится через `kepler.ark.subscribe("command_invoked")` в main.ts.
// Этот метод регистрирует in-memory listener'ы.
// ---------------------------------------------------------------------------

type EdenCommandChannel =
  | "eden:cmd:note:create"
  | "eden:cmd:note:search"
  | "eden:cmd:note:open-today";
const commandListeners = new Map<EdenCommandChannel, Set<(params: unknown) => void>>();
// Pending очередь — события, диспатчированные до того как App.vue
// зарегистрировал onCommand listener. Без буфера initialRoute()-event
// теряется (deep-link через `eden:note:open-today` приходит раньше mount).
const pendingDispatches = new Map<EdenCommandChannel, unknown[]>();

export function dispatchEdenCommand(channel: EdenCommandChannel, params: unknown): void {
  const set = commandListeners.get(channel);
  if (!set || set.size === 0) {
    const queue = pendingDispatches.get(channel) ?? [];
    queue.push(params);
    pendingDispatches.set(channel, queue);
    return;
  }
  for (const handler of set) {
    try {
      handler(params);
    } catch (e) {
      console.warn(`[eden-extension] command handler ${channel} threw:`, e);
    }
  }
}

function onCommand(channel: EdenCommandChannel, handler: (params: unknown) => void): () => void {
  let set = commandListeners.get(channel);
  if (!set) {
    set = new Set();
    commandListeners.set(channel, set);
  }
  set.add(handler);

  // Flush pending events для этого канала. Одноразово — после первого
  // listener'а очередь очищается. Используем microtask, чтобы caller
  // успел продолжить выполнение onMounted перед reentrant'ным вызовом
  // handler'а.
  const pending = pendingDispatches.get(channel);
  if (pending && pending.length > 0) {
    pendingDispatches.delete(channel);
    queueMicrotask(() => {
      for (const params of pending) {
        try {
          handler(params);
        } catch (e) {
          console.warn(`[eden-extension] flushed handler ${channel} threw:`, e);
        }
      }
    });
  }

  return () => {
    set?.delete(handler);
  };
}

// ---------------------------------------------------------------------------
// task_obj sync (Eden TipTap TaskList → Delphi).
// ---------------------------------------------------------------------------
//
// TipTap TaskItem ноды представляются в ARK как task_obj — те же объекты,
// которыми оперирует Delphi. Eden — write-source через `propsJson.source_app
// = "eden"` + `propsJson.source_note_id`. propsJson schema совпадает с
// `products/delphi/src/lib/electron-api-shim.ts::todoToArkTaskObject`,
// чтобы Delphi UI читал эти задачи без специальной логики.
//
// Связь: один taskItem ↔ один task_obj по `id == taskId` (UUID v4 из TipTap
// attribute). Idempotent через `upsert_object`.

const EDEN_TASK_OBJECT_TYPE_ID = "task_obj";

let taskObjectTypeRegisterPromise: Promise<void> | null = null;

export function ensureTaskObjectTypeRegistered(): Promise<void> {
  if (taskObjectTypeRegisterPromise) return taskObjectTypeRegisterPromise;
  const now = new Date().toISOString();
  taskObjectTypeRegisterPromise = ark("upsert_object_type", {
    object_type: {
      id: EDEN_TASK_OBJECT_TYPE_ID,
      name: "Задача",
      schemaJson: "{}",
      uiSchemaJson: "{}",
      systemLocked: false,
      createdAt: now,
      updatedAt: now,
    },
  })
    .then(() => undefined)
    .catch((err) => {
      taskObjectTypeRegisterPromise = null;
      console.warn("[eden-extension] task_obj type register failed:", err);
      throw err;
    });
  return taskObjectTypeRegisterPromise;
}

/**
 * Получить task_obj из ARK. null если не найден или soft-deleted.
 * Используется TaskRef NodeView для live-render'а.
 */
export async function getTask(taskId: string): Promise<ArkObjectRecord | null> {
  const obj = await ark<ArkObjectRecord | null>("get_object", { id: taskId });
  if (!obj) return null;
  if (obj.typeId !== EDEN_TASK_OBJECT_TYPE_ID) return null;
  if (obj.deletedAt) return null;
  return obj;
}

/**
 * Обновить отдельные поля task_obj (title или is_completed). Подтягивает
 * существующий объект, merge'ит, отправляет upsert. Используется TaskRef
 * NodeView когда юзер toggle'ит checkbox или меняет title inline.
 */
export async function patchTask(
  taskId: string,
  patch: {
    title?: string;
    isCompleted?: boolean;
    status?: TaskStatus;
  },
): Promise<void> {
  const existing = await ark<ArkObjectRecord | null>("get_object", { id: taskId });
  if (!existing) {
    console.warn("[eden-extension] patchTask: object not found", taskId);
    return;
  }
  const now = new Date().toISOString();
  const props = (existing.propsJson ?? {}) as Record<string, unknown>;
  const nextProps = { ...props };

  // Resolve next status. Если caller передал status — это source of truth.
  // Если только isCompleted — derive todo↔done. Иначе оставить existing.
  let nextStatus: TaskStatus;
  if (patch.status !== undefined) {
    nextStatus = patch.status;
  } else if (patch.isCompleted !== undefined) {
    nextStatus = patch.isCompleted ? "done" : "todo";
  } else {
    nextStatus = normalizeStatus({
      status: props.status,
      is_completed: props.is_completed,
      is_cancelled: props.is_cancelled,
    });
  }

  nextProps.status = nextStatus;
  nextProps.is_completed = deriveCompletedFlag(nextStatus);
  nextProps.is_cancelled = deriveCancelledFlag(nextStatus);
  if (deriveCompletedFlag(nextStatus) && !props.completed_at) {
    nextProps.completed_at = now;
  } else if (!deriveCompletedFlag(nextStatus)) {
    nextProps.completed_at = null;
  }
  if (deriveCancelledFlag(nextStatus) && !props.cancelled_at) {
    nextProps.cancelled_at = now;
  } else if (!deriveCancelledFlag(nextStatus)) {
    nextProps.cancelled_at = null;
  }

  // Если patch явно указал title — нормализуем (пустой/whitespace → placeholder),
  // иначе оставляем existing. Без этого Delphi показывает empty-row.
  const nextTitle =
    patch.title !== undefined ? patch.title.trim() || EDEN_EMPTY_TASK_TITLE : existing.title;
  await ark("upsert_object", {
    object: {
      ...existing,
      title: nextTitle,
      propsJson: nextProps,
      updatedAt: now,
    },
  });
}

/**
 * Создать новый task_obj (для slash-команды /задача или markdown `- [ ]`).
 * Возвращает taskId. Если caller передал `explicitId` — используется он
 * (нужно для input rule: synchronously генерируем UUID + вставляем TaskRef
 * node, потом async создаём task_obj с тем же id).
 */
/**
 * Default title для свежесозданной задачи без явного текста — чтобы в Delphi
 * не висели полностью пустые task_obj когда юзер не успел дописать. Юзер
 * либо переименует inline (autoFocus → input в TaskRefView), либо оставит
 * этот placeholder; Delphi покажет читаемое название вместо пустоты.
 */
const EDEN_EMPTY_TASK_TITLE = "Пустая задача";

// Derived flags from status — single source of truth: propsJson.status.
// is_completed / is_cancelled остаются для Delphi back-compat (Delphi
// читает их в `arkTaskObjectToTodo`).
function deriveCompletedFlag(status: TaskStatus): boolean {
  return status === "done";
}
function deriveCancelledFlag(status: TaskStatus): boolean {
  return status === "canceled";
}

export async function createTask(
  sourceNoteId: string,
  title = "",
  explicitId?: string,
): Promise<string> {
  await ensureTaskObjectTypeRegistered();
  const taskId = explicitId ?? crypto.randomUUID();
  const now = new Date().toISOString();
  const effectiveTitle = title.trim() || EDEN_EMPTY_TASK_TITLE;
  await ark("upsert_object", {
    object: {
      id: taskId,
      typeId: EDEN_TASK_OBJECT_TYPE_ID,
      title: effectiveTitle,
      contentJson: writeEntryMarkdown(""),
      propsJson: {
        description: null,
        priority: 0,
        scheduled_date: null,
        deadline: null,
        reminder_date: null,
        is_today: false,
        is_evening: false,
        is_someday: false,
        is_completed: false,
        completed_at: null,
        is_cancelled: false,
        cancelled_at: null,
        is_trashed: false,
        // Linear-style status (2026-05-20): single source of truth для
        // жизненного цикла задачи. is_completed/is_cancelled derive'ятся
        // из status в patchTask. Default — triage (новая задача требует
        // сортировки перед попаданием в активный todo-лист).
        status: "triage",
        sort_order: 0,
        heading_id: null,
        project_id: null,
        area_id: null,
        tag_ids: [],
        checklist_items: [],
        recurrence_rule: null,
        billable: false,
        price: null,
        created_at: now,
        source_app: "eden",
        source_note_id: sourceNoteId,
        model_version: 1,
      },
      createdAt: now,
      updatedAt: now,
      deletedAt: null,
    },
  });
  return taskId;
}

/**
 * Подписаться на ARK events `object_upserted` / `object_deleted`. Возвращает
 * unsubscribe. Filter колбэк вызывается с `{event, id, type_id?}`.
 *
 * До 2026-05-20 ws_server не форвардил ark-core events клиентам — было
 * fixed вместе с этим коммитом. Eden TaskRef NodeView подписывается через
 * это API чтобы live-обновлять чекбокс/title когда Delphi пишет task_obj.
 */
export function subscribeObjectChanges(
  handler: (payload: {
    event: "object_upserted" | "object_deleted";
    id: string;
    typeId?: string;
  }) => void,
): () => void {
  const bridge = keplerBridge();
  const offU = bridge.subscribe("object_upserted", (payload) => {
    const p = payload as { id?: string; type_id?: string };
    if (typeof p.id === "string")
      handler({ event: "object_upserted", id: p.id, typeId: p.type_id });
  });
  const offD = bridge.subscribe("object_deleted", (payload) => {
    const p = payload as { id?: string };
    if (typeof p.id === "string") handler({ event: "object_deleted", id: p.id });
  });
  return () => {
    offU();
    offD();
  };
}

export async function softDeleteTask(taskId: string): Promise<void> {
  // ARK delete_object is a hard delete. Backspace на пустом TaskRef в Eden
  // должен только скрыть task_obj из активных списков.
  try {
    const existing = await ark<ArkObjectRecord | null>("get_object", { id: taskId });
    if (!existing || existing.deletedAt) return;
    const deletedAt = millisToArkTimestamp(Date.now());
    await ark("upsert_object", {
      object: {
        ...existing,
        updatedAt: deletedAt,
        deletedAt,
      },
    });
  } catch (err) {
    console.warn("[eden-extension] softDeleteTask failed:", taskId, err);
    throw err;
  }
}

// ---------------------------------------------------------------------------
// Install.
// ---------------------------------------------------------------------------

export function installKeplerApiShim(): void {
  (window as unknown as { api: Window["api"] }).api = {
    saveEntry,
    loadEntry,
    listEntries,
    getVaultPath,
    getRecentVaultPaths,
    selectFolder,
    setVaultPath,
    exportMarkdownVault: saveMarkdownVault,
    openMarkdownFile,
    saveMarkdownFile,
    openMarkdownVault,
    searchEntries,
    createFolder,
    listFolders,
    listNoteTypes,
    ensureCollectionObjects,
    saveNoteType,
    deleteNoteType,
    moveEntryToFolder,
    moveFolderToFolder,
    deleteEntry,
    deleteFolder,
    getSidebarConfig,
    updateSidebarConfig,
    getEdenVisibleObjectTypeIds,
    setEdenVisibleObjectTypeIds,
    getPlatform,
    listTrashEntries,
    restoreEntry,
    permanentDeleteEntry,
    purgeExpiredTrash,
    getVaultStorageInfo,
    getDiskFreeSpace,
    zoomGet,
    zoomSet,
    minimize,
    maximize,
    close,
    onCommand,
  };
}
