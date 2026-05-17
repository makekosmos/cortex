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

function arkTimestampToMillis(value?: string | null): number {
  if (!value) return Date.now();
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : Date.now();
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
  const headerProps = {
    ...object.propsJson,
    related_notes: links
      .filter((l) => l.sourceObjectId === object.id && l.linkType === "related")
      .map((l) => l.targetObjectId),
  };

  return normalizeEntry({
    id: object.id,
    title: object.title,
    content_json: JSON.stringify(
      object.contentJson ?? { type: "doc", content: [{ type: "paragraph" }] },
    ),
    created_at: arkTimestampToMillis(object.createdAt),
    updated_at: arkTimestampToMillis(object.updatedAt),
    folder_id: null,
    type_id: object.typeId,
    header_layout: objectType
      ? (parseNoteTypeUiSchema(objectType.uiSchemaJson).header_layout ?? "default")
      : "default",
    header_props_json: stringifyHeaderProps(headerProps),
    schema_version: 1,
    deleted_at: object.deletedAt ? arkTimestampToMillis(object.deletedAt) : null,
  });
}

function mapEntryToArkObject(entry: Entry): ArkObjectRecord {
  const headerProps = parseHeaderPropsJson(entry.header_props_json);
  const { related_notes: _ignored, ...propsJson } = headerProps;
  let contentJson: unknown = { type: "doc", content: [{ type: "paragraph" }] };
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
      created_at: arkTimestampToMillis(objectType.createdAt),
      updated_at: arkTimestampToMillis(objectType.updatedAt),
    }),
  );
}

function mapNoteTypeToArkObjectType(noteType: NoteType): ArkObjectTypeRecord {
  const uiSchema = parseNoteTypeUiSchema(noteType.ui_schema_json);
  const definition = parseNoteTypeDefinition(noteType.schema_json);
  const featuredFromUi = uiSchema.featured_fields ?? [];
  const visibleFromFields = definition.fields
    .filter((f) => f.visible !== false)
    .map((f) => f.id);
  const readOnlyFromFields = definition.fields
    .filter((f) => f.read_only === true)
    .map((f) => f.id);

  return {
    id: noteType.id,
    name: noteType.name,
    schemaJson: noteType.schema_json,
    uiSchemaJson: JSON.stringify({
      featured_fields: featuredFromUi,
      visible_fields: uiSchema.visible_fields?.length
        ? uiSchema.visible_fields
        : visibleFromFields,
      hidden_fields: uiSchema.hidden_fields ?? ["created_at", "updated_at", "deleted_at"],
      read_only_fields: uiSchema.read_only_fields?.length
        ? uiSchema.read_only_fields
        : readOnlyFromFields,
      field_order:
        uiSchema.field_order?.length ? uiSchema.field_order : definition.fields.map((f) => f.id),
      header_layout: uiSchema.header_layout ?? "inline",
      default_layout: uiSchema.default_layout ?? "page",
      default_template_id: uiSchema.default_template_id ?? null,
    }),
    createdAt: millisToArkTimestamp(noteType.created_at),
    updatedAt: millisToArkTimestamp(noteType.updated_at),
    systemLocked: noteType.id === "note_obj" || noteType.id === "game_obj",
  };
}

// ---------------------------------------------------------------------------
// ARK-backed operations
// ---------------------------------------------------------------------------

export async function listEntries(): Promise<Entry[]> {
  const [objects, links, objectTypes] = await Promise.all([
    ark<unknown>("list_objects").then(ensureList<ArkObjectRecord>),
    ark<unknown>("list_object_links").then(ensureList<ArkObjectLinkRecord>),
    ark<unknown>("list_object_types").then(ensureList<ArkObjectTypeRecord>),
  ]);

  const typesById = new Map(objectTypes.map((t) => [t.id, t]));
  return objects
    .map((o) => mapArkObjectToEntry(o, links, typesById.get(o.typeId)))
    .sort((a, b) => b.updated_at - a.updated_at);
}

export async function loadEntry(id: string): Promise<Entry | undefined> {
  const [object, links, objectTypes] = await Promise.all([
    ark<ArkObjectRecord | null>("get_object", { id }),
    ark<unknown>("list_object_links").then(ensureList<ArkObjectLinkRecord>),
    ark<unknown>("list_object_types").then(ensureList<ArkObjectTypeRecord>),
  ]);

  if (!object) return undefined;
  const typesById = new Map(objectTypes.map((t) => [t.id, t]));
  return mapArkObjectToEntry(object, links, typesById.get(object.typeId));
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
  const existingEntries = await listEntries();
  const conflicting = existingEntries.find((candidate) => {
    if (candidate.id === normalized.id || candidate.deleted_at !== null) return false;
    if ((candidate.folder_id ?? null) !== (normalized.folder_id ?? null)) return false;
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
  if (!existing) {
    return {
      ok: false,
      reason: "entry_not_found",
      message: "Заметка не найдена в ARK",
    };
  }
  await ark<boolean>("delete_object", { id: entryId });
  return { ok: true, entryId };
}

export async function listNoteTypes(): Promise<NoteType[]> {
  const types = await ark<unknown>("list_object_types").then(
    ensureList<ArkObjectTypeRecord>,
  );
  return types.map(mapArkObjectTypeToNoteType);
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
  folderId: string,
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

async function exportMarkdownVault(): Promise<ExportMarkdownVaultResult | null> {
  return null;
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

async function permanentDeleteEntry(
  entryId: string,
): Promise<{ ok: boolean; entryId?: string }> {
  // ARK `delete_object` уже выполняет soft-delete; для extension'а трактуем
  // «удалить навсегда» как повторный hard-delete — backend выкидывает запись
  // окончательно через тот же endpoint после soft-state. Полноценный hard-purge
  // endpoint в roadmap (Phase 7+ purge tooling в Kepler shell).
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
  const raw = localStorage.getItem(ZOOM_STORAGE_KEY);
  const parsed = raw ? parseFloat(raw) : NaN;
  return Number.isFinite(parsed) ? parsed : 1;
}

async function zoomSet(factor: number): Promise<number> {
  const clamped = Math.max(0.5, Math.min(2.0, factor));
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

type EdenCommandChannel = "eden:cmd:note:create" | "eden:cmd:note:search";
const commandListeners = new Map<EdenCommandChannel, Set<(params: unknown) => void>>();

export function dispatchEdenCommand(channel: EdenCommandChannel, params: unknown): void {
  const set = commandListeners.get(channel);
  if (!set) return;
  for (const handler of set) {
    try {
      handler(params);
    } catch (e) {
      console.warn(`[eden-extension] command handler ${channel} threw:`, e);
    }
  }
}

function onCommand(
  channel: EdenCommandChannel,
  handler: (params: unknown) => void,
): () => void {
  let set = commandListeners.get(channel);
  if (!set) {
    set = new Set();
    commandListeners.set(channel, set);
  }
  set.add(handler);
  return () => {
    set?.delete(handler);
  };
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
    exportMarkdownVault,
    searchEntries,
    createFolder,
    listFolders,
    listNoteTypes,
    saveNoteType,
    deleteNoteType,
    moveEntryToFolder,
    moveFolderToFolder,
    deleteEntry,
    deleteFolder,
    getSidebarConfig,
    updateSidebarConfig,
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
