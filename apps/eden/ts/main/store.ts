import fs from "node:fs";

import path from "node:path";

import { app } from "electron";

import { defaultCodeToolsSettings, type CodeToolsSettings } from "./codeTools";

import { runHeartRequest } from "./heart";

import {
  normalizeSlug,
  noteTypeSchema,
  parseHeaderTemplate,
  parseNoteTypeDefinition,
  validateHeaderProps,
  type NoteType,
} from "@/lib/typedNotes";

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
  return noteTypeSchema.parse(noteType);
}

async function getNoteTypeById(noteTypeId: string) {
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
  return readAppConfig().vaultPath || null;
}

export function getRecentVaultPaths(): string[] {
  const config = readAppConfig();

  return normalizeRecentVaultPaths(
    config.recentVaultPaths ?? [],
    config.vaultPath ?? null,
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

  return true;
}

export async function setVaultPath(newPath: string) {
  const config = readAppConfig();

  writeAppConfig({
    ...config,

    vaultPath: newPath,

    recentVaultPaths: normalizeRecentVaultPaths(
      [newPath, ...(config.recentVaultPaths ?? [])],

      newPath,
    ),
  });

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

  const normalizedEntry = normalizeEntry(entry);

  const validationResult = await validateEntryTypeMetadata(normalizedEntry);

  if (validationResult) {
    return validationResult;
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

  if (!vaultPath) {
    return [];
  }

  const entries = await runHeartRequest<Entry[]>({
    operation: "list_entries",

    vaultPath,
  });

  return entries.map(normalizeEntry);
}

export async function searchEntries(query: string): Promise<SearchResult[]> {
  const vaultPath = currentVaultPath ?? getVaultPath();

  if (!vaultPath || !query.trim()) {
    return [];
  }

  return runHeartRequest<SearchResult[]>({
    operation: "search_entries",

    vaultPath,

    query,
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

  if (!vaultPath) {
    return [];
  }

  const noteTypes = await runHeartRequest<NoteType[]>({
    operation: "list_note_types",

    vaultPath,
  });

  return noteTypes.map(normalizeNoteType);
}

export async function saveNoteType(
  noteType: NoteType,
): Promise<SaveNoteTypeResult> {
  const vaultPath = requireVaultPath();

  try {
    parseNoteTypeDefinition(noteType.schema_json);

    parseHeaderTemplate(noteType.header_template_json);
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

  return runHeartRequest<SaveNoteTypeResult>({
    operation: "save_note_type",

    vaultPath,

    note_type: normalizedNoteType,
  });
}

export async function deleteNoteType(noteTypeId: string) {
  const vaultPath = requireVaultPath();

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
